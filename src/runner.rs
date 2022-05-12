use std::error::Error;
use std::path::PathBuf;

use async_std::fs::read_to_string;
use glob::glob;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};

use crate::channel::Channel;

#[derive(Serialize, Debug)]
pub struct Runner {
    pub id:              String,
    #[serde(rename = "runnerScript")]
    pub script:          String,
    #[serde(rename = "canUseChannel")]
    pub can_use_channel: Vec<String>,
    #[serde(rename = "requiredFields")]
    pub required_fields: Vec<String>,
    #[serde(skip_serializing)]
    pub schema:          JSONSchema,
}

impl<'de> Deserialize<'de> for Runner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct R {
            pub id:              String,
            #[serde(rename = "runnerScript")]
            pub script:          String,
            #[serde(rename = "canUseChannel")]
            pub can_use_channel: Vec<String>,
            #[serde(rename = "requiredFields")]
            pub required_fields: Vec<String>,
        }
        let R {
            can_use_channel,
            script,
            id,
            required_fields,
        } = <R as Deserialize>::deserialize(deserializer)?;

        let schema = json!({
            "type": "object",
            "required": required_fields,
        });

        let schema = JSONSchema::compile(&schema).expect("valid schema");

        Ok(Runner {
            id,
            schema,
            required_fields,
            can_use_channel,
            script,
        })
    }
}

pub async fn parse_runners(path: &str, channels: &Vec<Channel>) -> Vec<Runner> {
    let mut runners = Vec::new();
    let mut iterator = glob(path)
        .expect("Failed to read channels glob pattern")
        .flatten()
        .map(parse_runner);

    let channel_exists = |id: &str| channels.iter().any(|c| &c.id == id);

    while let Some(item) = iterator.next() {
        match item.await {
            Ok(runner) => {
                if runner.can_use_channel.iter().fold(
                    true,
                    |acc, channel_id| {
                        if !channel_exists(channel_id) {
                            eprintln!("No such channel found! {}", channel_id);
                            false
                        } else {
                            acc
                        }
                    },
                ) {
                    runners.push(runner);
                }
            }
            Err(e) => eprintln!("Parsing channel failed '{}'", e),
        }
    }

    runners
}

pub async fn parse_runner(path: PathBuf) -> Result<Runner, Box<dyn Error>> {
    let file = read_to_string(path).await?;
    let channel: Runner = serde_json::from_str(&file)?;
    Ok(channel)
}
