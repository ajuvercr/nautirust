use std::path::{Path, PathBuf};

use crate::{
    channel::Channel,
    runner::{self, Runner},
};
use async_std::fs::read_to_string;
use serde::{Deserialize, Serialize};

use super::start_subproc;

#[derive(Serialize, Deserialize, Debug)]
struct StopConfig {
    id: String,
    #[serde(rename = "stopScript")]
    script: String,

    location: String,
}

/// Gracefully stop the runners and channels specified in the config
#[derive(clap::Args, Debug)]
pub struct Command {
    file: String,
}

impl Command {
    pub async fn execute(self, _channels: Vec<Channel>, runners: Vec<Runner>) {
        runners.into_iter().rev().for_each(|r| {
            let mut proc =
                start_subproc(&r.stop_script, r.location.as_ref().unwrap());
            proc.wait().unwrap();
        });
    }
}
