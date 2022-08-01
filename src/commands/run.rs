use std::env;
use std::path::Path;

use async_std::fs::{self, read_to_string, write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempdir::TempDir;

use crate::channel::Channel;
use crate::commands::generate::Args;
use crate::runner::Runner;
use crate::step::Step;

#[derive(Serialize, Deserialize, Debug)]
pub struct RunThing {
    #[serde(rename = "processorConfig")]
    pub processor_config: Step,
    args: Args,
    #[serde(flatten)]
    rest: Value,
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
    file: String,
    /// temporary directory to put step configuration files
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
        let values: Steps = serde_json::from_str(&content).unwrap();

        let mut procs = Vec::new();

        for value in values.steps {
            let file = path.join(format!("{}.json", value.processor_config.id));
            let config = serde_json::to_vec_pretty(&value).unwrap();

            write(file.clone(), config).await.unwrap();

            let runner = runners
                .iter()
                .find(|r| r.id == value.processor_config.runner_id)
                .unwrap();

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

            let proc = super::start_subproc(
                command,
                runner.location.as_ref(),
                &value.processor_config.id,
                false,
            );

            procs.extend(proc);
        }

        for (mut proc, h1, h2) in procs {
            proc.wait().unwrap();
            h1.join().unwrap();
            h2.join().unwrap();
        }
    }
}
