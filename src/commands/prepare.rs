
use async_std::fs::read_to_string;
use serde::{Deserialize, Serialize};

use crate::channel::Channel;
use crate::runner::Runner;

#[derive(Serialize, Deserialize, Debug)]
struct PrepareConfig {
    id:       String,
    #[serde(rename = "stopScript")]
    script:   String,
    location: String,
}

// Prepares the execution pipeline by starting the required channels/services defined in the config file
#[derive(clap::Args, Debug)]
pub struct Command {
    file: String,
}

impl Command {
    pub async fn execute(self, _channels: Vec<Channel>, _runners: Vec<Runner>) {
        let content = read_to_string(self.file).await.unwrap();

        let used_channels_map = super::get_used_channels(content, &_channels);
        used_channels_map
            .into_values()
            .into_iter()
            .for_each(|chan| {
                super::start_subproc(
                    chan.start.as_ref().unwrap(),
                    chan.location.as_ref().unwrap(),
                );
            })
    }
}
