use std::process::Child;

use async_std::fs::read_to_string;

use super::run::{RunThing, Steps};
use crate::channel::Channel;
use crate::runner::Runner;
use crate::step::Step;

/// Prepares the execution pipeline by starting the required channels/runner
#[derive(clap::Args, Debug)]
pub struct Command {
    /// Config file
    file: String,
}

impl Command {
    pub async fn execute(self, channels: Vec<Channel>, runners: Vec<Runner>) {
        let content = read_to_string(self.file).await.unwrap();
        let values: Steps = serde_json::from_str(&content).unwrap();

        let mut procs: Vec<Child> = Vec::new();
        let used_channels = super::get_used_channels(&content, &channels);
        used_channels.for_each(
            |Channel {
                 start, location, ..
             }| {
                super::add_add_subproc(start, location.as_ref(), &mut procs)
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
                 ref start,
                 ..
             }| {
                super::add_add_subproc(start, location.as_ref(), &mut procs)
            },
        );

        values.steps.iter().for_each(
            |RunThing {
                 processor_config:
                     Step {
                         build, location, ..
                     },
                 ..
             }| {
                super::add_add_subproc(build, location.as_ref(), &mut procs);
            },
        );

        // Stops the processors in the reverse order
        while !procs.is_empty() {
            procs.pop().unwrap().wait().unwrap();
        }
    }
}
