use std::{
    path::{Path, PathBuf},
    process::Child,
};

use crate::{
    channel::Channel,
    runner::{self, Runner},
};
use async_std::fs::read_to_string;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{run::Values, start_subproc};

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
        for value in values.values {
            let runner = runners
                .iter()
                .find(|r| r.id == value.processor_config.runner_id)
                .unwrap();
            
            let proc =  super::start_subproc(&runner.stop_script, runner.location.as_ref().unwrap());

            procs.push(proc);
        }



        // Stops the processors in the reverse order
        while !procs.is_empty() {

            procs.pop().unwrap().wait().unwrap(); 

        }

    }
}
