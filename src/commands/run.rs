use std::env;
use std::path::Path;
use std::process::Child;

use async_std::fs::{self, read_to_string, write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempdir::TempDir;

use crate::channel::Channel;
use crate::runner::Runner;

#[derive(Serialize, Deserialize, Debug)]
struct ProcConfig {
    id:        String,
    #[serde(rename = "runnerId")]
    runner_id: String,
    #[serde(flatten)]
    other:     Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct RunThing {
    #[serde(rename = "processorConfig")]
    processor_config: ProcConfig,

    #[serde(flatten)]
    other: Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct Values {
    values: Vec<RunThing>,
}

/// Run the actual configs
#[derive(clap::Args, Debug)]
pub struct Command {
    file:    String,
    /// tmpdir to put temporary files
    #[clap(short, long)]
    tmp_dir: Option<String>,
}

impl Command {
    pub(crate) async fn execute(
        self,
        _channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let mut tmp_dir = None;

        if let Some(l) = &self.tmp_dir {
            fs::create_dir_all(l).await.unwrap();
        }

        let path = self
            .tmp_dir
            .as_ref()
            .map(|l| Path::new(l).to_owned())
            .unwrap_or_else(|| {
                let tmp = TempDir::new("orchestrator").unwrap();
                let out = tmp.path().to_owned();
                tmp_dir = Some(tmp);
                out
            });

        let content = read_to_string(self.file).await.unwrap();
        let values: Values = serde_json::from_str(&content).unwrap();

        let mut procs: Vec<Child> = Vec::new();

        for value in values.values {
            let file = path.join(format!("{}.json", value.processor_config.id));
            let config = serde_json::to_vec_pretty(&value).unwrap();

            write(file.clone(), config).await.unwrap();

            let runner = runners
                .iter()
                .find(|r| r.id == value.processor_config.runner_id)
                .unwrap();

            let mut command = shlex::split(&runner.script).unwrap();

            command.iter_mut().for_each(|part| {
                if part == "{config}" {
                    *part = file
                        .canonicalize()
                        .expect("Couldn't canonicalize path :(")
                        .display()
                        .to_string()
                }

                if part == "{cwd}" {
                    *part = env::current_dir().unwrap()
                        .canonicalize()
                        .expect("Couldn't canonicalize path :(")
                        .display()
                        .to_string()
                }
            });

            println!("spawning {}", command.join(" "));

            let proc = if let Some(l) = &runner.location {
                std::process::Command::new(&command[0])
                    .args(&command[1..])
                    .current_dir(l)
                    .spawn()
                    .unwrap()
            } else {
                std::process::Command::new(&command[0])
                    .args(&command[1..])
                    .spawn()
                    .unwrap()
            };

            procs.push(proc);
        }

        for mut proc in procs {
            proc.wait().unwrap();
        }
    }
}
