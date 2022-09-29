use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Child;
use std::thread::JoinHandle;

use async_recursion::async_recursion;
use async_std::fs::{self, read_to_string, write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempdir::TempDir;

use super::OutputConfig;
use crate::channel::Channel;
use crate::runner::Runner;
use crate::step::{Output, Step, StepArgument, SubStep};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunThing {
    #[serde(rename = "processorConfig")]
    pub processor_config: Step,
    pub(crate) args:      HashMap<String, StepArgument>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Steps {
    #[serde(rename = "values")]
    pub steps: Vec<RunThing>,
}

/// Run a configured pipeline
#[derive(clap::Args, Debug)]
pub struct Command {
    /// Config file
    file:    String,
    /// temporary directory to put step configuration files
    #[clap(short, long)]
    tmp_dir: Option<String>,
}

struct RunHandler<'a> {
    tmp_dir:              PathBuf,
    runners:              &'a Vec<Runner>,
    sub_argument_outputs: HashMap<String, Value>,
}

impl<'a> RunHandler<'a> {
    fn from(command: &Command, runners: &'a Vec<Runner>) -> Self {
        let path = command
            .tmp_dir
            .as_ref()
            .map(|l| Path::new(l).to_owned())
            .unwrap_or_else(|| {
                let tmp = TempDir::new("orchestrator").unwrap();
                tmp.path().to_owned()
            });

        Self {
            sub_argument_outputs: HashMap::default(),
            runners,
            tmp_dir: path,
        }
    }

    fn get_runner(&self, id: &str) -> &'a Runner {
        self.runners.iter().find(|r| r.id == id).unwrap()
    }

    fn get_tmp_file(&self, id: &str) -> PathBuf {
        self.tmp_dir.join(format!("{}.json", id))
    }

    #[async_recursion]
    async fn arg_to_value(&mut self, arg: StepArgument) -> Option<Value> {
        if let StepArgument::Step {
            sub:
                SubStep {
                    run,
                    serialization,
                    output,
                },
        } = arg
        {
            if let Some(value) =
                self.sub_argument_outputs.get(&run.processor_config.id)
            {
                return value.clone().into();
            }

            let process_config_id = run.processor_config.id.clone();
            let (mut child, stdout, stderr) = run_thing(
                run,
                self,
                OutputConfig {
                    stdout: true,
                    stderr: true,
                },
            )
            .await?;

            child.wait().ok()?;

            let stdout = stdout.join().ok()?;
            let stderr = stderr.join().ok()?;

            let (content, terminator) = match output {
                Output::Stdout => (stdout, ".stdout"),
                Output::Stderr => (stderr, ".stderr"),
            };

            let path = self
                .get_tmp_file(&format!("{}{}", process_config_id, terminator));
            write(&path, content).await.ok()?;

            let out = StepArgument::File {
                path: path.to_string_lossy().to_string(),
                serialization,
            };

            let value = serde_json::to_value(out).ok()?;
            self.sub_argument_outputs
                .insert(process_config_id, value.clone());
            value.into()
        } else {
            serde_json::to_value(arg).ok()
        }
    }
}

async fn run_thing(
    run: RunThing,
    handler: &mut RunHandler<'_>,
    output: OutputConfig,
) -> Option<(Child, JoinHandle<String>, JoinHandle<String>)> {
    #[derive(Serialize)]
    struct SimpleRun<'a> {
        #[serde(rename = "processorConfig")]
        pub processor_config: &'a Step,
        args:                 HashMap<String, Value>,
    }

    let mut args = HashMap::new();
    for (k, v) in run.args {
        args.insert(k, handler.arg_to_value(v).await?);
    }

    let name = &run.processor_config.id;
    let runner = handler.get_runner(&run.processor_config.runner_id);

    let run = SimpleRun {
        processor_config: &run.processor_config,
        args,
    };

    let config = serde_json::to_string_pretty(&run).ok()?;
    run_value(config, handler.get_tmp_file(name), runner, name, output).await
}

async fn run_value(
    config: String,
    file: PathBuf,
    runner: &Runner,
    name: &str,
    output: OutputConfig,
) -> Option<(Child, JoinHandle<String>, JoinHandle<String>)> {
    write(file.clone(), config).await.unwrap();

    let config_path = format!(
        "'{}'",
        file.canonicalize().expect("canonicalize path").display()
    );

    let current_dir = format!(
        "'{}'",
        env::current_dir()
            .unwrap()
            .canonicalize()
            .expect("canonicalize path")
            .display()
    );

    let command = runner
        .script
        .clone()
        .replace("{config}", &config_path)
        .replace("{cwd}", &current_dir);

    super::start_subproc(command, runner.location.as_ref(), name, output)
}

impl Command {
    pub(crate) async fn execute(
        self,
        _channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let content = read_to_string(&self.file).await.unwrap();
        let values: Steps = serde_json::from_str(&content).unwrap();

        let mut procs = Vec::new();

        let mut handler = RunHandler::from(&self, &runners);
        fs::create_dir_all(&handler.tmp_dir).await.unwrap();

        for value in values.steps {
            let proc = run_thing(value, &mut handler, OutputConfig::default())
                .await
                .expect("");
            procs.push(proc);
        }

        for (mut proc, h1, h2) in procs {
            proc.wait().unwrap();
            h1.join().unwrap();
            h2.join().unwrap();
        }
    }
}
