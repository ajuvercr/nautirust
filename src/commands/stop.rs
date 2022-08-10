use async_std::fs::read_to_string;

use super::OutputConfig;
use super::run::Steps;
use crate::channel::Channel;
use crate::runner::Runner;

/// Gracefully stop the runners and channels specified in the config
#[derive(clap::Args, Debug)]
pub struct Command {
    /// Config file
    file: String,
}

impl Command {
    pub async fn execute(self, _channels: Vec<Channel>, runners: Vec<Runner>) {
        let content = read_to_string(self.file).await.unwrap();
        let values: Steps = serde_json::from_str(&content).unwrap();

        let mut procs = Vec::new();
        let used_channels = super::get_used_channels(&content, &_channels);

        used_channels.for_each(
            |Channel {
                 stop, location, id, ..
             }| {
                super::add_add_subproc(
                    stop,
                    location.as_ref(),
                    &mut procs,
                    id,
                    OutputConfig::default(),
                )
            },
        );

        let used_runners = runners.iter().filter(|runner| {
            values
                .steps
                .iter()
                .any(|v| v.processor_config.runner_id == runner.id)
        });

        used_runners.for_each(
            |Runner {
                 ref location,
                 ref stop,
                 id,
                 ..
             }| {
                super::add_add_subproc(
                    stop,
                    location.as_ref(),
                    &mut procs,
                    id,
                    OutputConfig::default(),
                )
            },
        );

        // Stops the processors in the reverse order
        while !procs.is_empty() {
            let (mut proc, h1, h2) = procs.pop().unwrap();
            proc.wait().unwrap();
            h1.join().unwrap();
            h2.join().unwrap();
        }
    }
}
