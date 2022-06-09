use std::process::Child;

use async_std::fs::read_to_string;

use super::run::Values;
use crate::channel::Channel;
use crate::runner::{Runner};

/// Gracefully stop the runners and channels specified in the config
#[derive(clap::Args, Debug)]
pub struct Command {
    file: String,
}

impl Command {
    pub async fn execute(self, _channels: Vec<Channel>, runners: Vec<Runner>) {
        let content = read_to_string(self.file).await.unwrap();
        let values: Values = serde_json::from_str(&content).unwrap();

        let mut procs: Vec<Child> = Vec::new();
        let used_channels = super::get_used_channels(content, &_channels);

        for chan in used_channels.values(){
            let stop_procs =  super::start_subproc( chan.stop.as_ref().unwrap(), chan.location.as_ref().unwrap());
            procs.extend(stop_procs);
        }


        for value in values.values {
            let runner = runners
                .iter()
                .find(|r| r.id == value.processor_config.runner_id)
                .unwrap();

            let proc = super::start_subproc(
                &runner.stop_script,
                runner.location.as_ref().unwrap(),
            );

            procs.extend(proc);
        }

        // Stops the processors in the reverse order
        while !procs.is_empty() {
            procs.pop().unwrap().wait().unwrap();
        }
    }
}
