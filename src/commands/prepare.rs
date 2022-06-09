use std::collections::{HashMap, HashSet};

use async_std::fs::read_to_string;
use jsonpath_rust::{JsonPathFinder, JsonPathQuery};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::start_subproc;
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

        let used_channels_map = get_used_channels(content, &_channels);
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

fn get_used_channels(
    content: String,
    channels: &[Channel],
) -> HashMap<String, &Channel> {
    let json: Value = serde_json::from_str(&content).unwrap();
    let channel_types = json.path("$..type").unwrap();
    let types: Vec<_> = channel_types
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|f| f.as_str())
        .collect();

    let mut used_channels = HashMap::new();
    for typ in types {
        for chan in channels {
            if chan.id == typ {
                used_channels.insert(chan.id.to_owned(), chan);
                break;
            }
        }
    }

    used_channels
}
