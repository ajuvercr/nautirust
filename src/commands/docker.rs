use std::collections::HashSet;
use std::env;
use std::path::Path;

use async_std::fs::{self, read_to_string, write};
use tempdir::TempDir;

use super::run::Steps;
use crate::channel::Channel;
use crate::runner::Runner;

/// Create a docker-compose file from a nautirust pipeline
#[derive(clap::Args, Debug)]
pub struct Command {
    /// Config file
    file: String,
    #[clap(short, long)]
    output: bool,
    /// temporary directory to put step configuration files
    #[clap(short, long)]
    tmp_dir: Option<String>,
}

impl Command {
    pub(crate) async fn execute(
        self,
        channels: Vec<Channel>,
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

        // Check if each runner can docker
        let find_runner = |id| runners.iter().find(|r| &r.id == id).unwrap();
        let runners_without_docker = values
            .steps
            .iter()
            .map(|s| find_runner(&s.processor_config.runner_id))
            .filter(|runner| runner.docker.is_none())
            .map(|r| &r.id)
            .collect::<HashSet<_>>();

        if !runners_without_docker.is_empty() {
            eprintln!(
                "Not all runners support dockerization ({:?})",
                runners_without_docker
            );
            return;
        }

        let mut procs = Vec::new();

        let used_channels = super::get_used_channels(&content, &channels);
        used_channels.for_each(
            |Channel {
                 docker,
                 location,
                 id,
                 ..
             }| {
                super::add_add_subproc(
                    docker,
                    location.as_ref(),
                    &mut procs,
                    id,
                    true,
                )
            },
        );

        for value in &values.steps {
            let file = path.join(format!("{}.json", value.processor_config.id));
            let config = serde_json::to_vec_pretty(&value).unwrap();

            write(file.clone(), config).await.unwrap();

            let runner = find_runner(&value.processor_config.runner_id);

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

            let script = runner
                .docker
                .clone()
                .unwrap()
                .replace("{config}", &config_path)
                .replace("{cwd}", &current_dir);

            super::add_add_subproc(
                &script.into(),
                runner.location.as_ref(),
                &mut procs,
                &value.processor_config.id,
                true,
            );
        }

        let docker_header = "services:\n";
        let docker_content: String = [docker_header.to_string()]
            .into_iter()
            .chain(
                procs
                    .into_iter()
                    .map(|(mut proc, h1, h2)| {
                        proc.wait().unwrap();
                        let output = h1.join().unwrap();
                        h2.join().unwrap();
                        output
                    })
            )
            .collect();

        if self.output {
            write("docker-compose.yml", docker_content).await.unwrap();
        } else {
            println!("{}", docker_content);
        }
    }
}
