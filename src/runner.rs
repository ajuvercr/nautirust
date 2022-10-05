use std::path::PathBuf;

use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct Runner {
    pub id:                    String,
    pub start:                 Option<String>,
    pub docker:                Option<String>,
    pub stop:                  Option<String>,
    #[serde(rename = "runnerScript")]
    pub script:                String,
    #[serde(rename = "canUseChannel")]
    pub can_use_channel:       Vec<String>,
    #[serde(rename = "requiredFields")]
    pub required_fields:       Vec<String>,
    #[serde(rename = "canUseSerialization")]
    pub can_use_serialization: Vec<String>,
    #[serde(skip_serializing)]
    pub schema:                JSONSchema,
    #[serde(skip_serializing)]
    pub location:              Option<PathBuf>,
}

impl<'de> Deserialize<'de> for Runner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct R {
            pub id:                    String,
            pub docker:                Option<String>,
            pub start:                 Option<String>,
            pub stop:                  Option<String>,
            #[serde(rename = "runnerScript")]
            pub script:                String,
            #[serde(rename = "canUseChannel")]
            pub can_use_channel:       Vec<String>,
            #[serde(rename = "requiredFields")]
            pub required_fields:       Vec<String>,
            #[serde(rename = "canUseSerialization")]
            pub can_use_serialization: Vec<String>,
        }

        let R {
            start,
            can_use_channel,
            can_use_serialization,
            docker,
            stop,
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
            start,
            schema,
            required_fields,
            docker,
            can_use_channel,
            can_use_serialization,
            stop,
            script,
            location: None,
        })
    }
}

#[cfg(feature = "io")]
pub use io::*;
#[cfg(feature = "io")]
mod io {
    use std::error::Error;
    use std::path::PathBuf;

    use glob::glob;

    use super::*;
    use crate::channel::Channel;

    pub async fn parse_runners(
        path: &str,
        channels: &[Channel],
    ) -> Vec<Runner> {
        let mut runners = Vec::new();
        let iterator = glob(path)
            .expect("Failed to read channels glob pattern")
            .flatten()
            .map(parse_runner);

        let channel_exists = |id: &str| channels.iter().any(|c| c.id == id);

        for item in iterator {
            match item.await {
                Ok(runner) => {
                    if runner.can_use_channel.iter().fold(
                        true,
                        |acc, channel_id| {
                            if !channel_exists(channel_id) {
                                eprintln!(
                                    "No such channel found! {}",
                                    channel_id
                                );
                                false
                            } else {
                                acc
                            }
                        },
                    ) {
                        runners.push(runner);
                    }
                }
                Err(e) => eprintln!("Parsing runner failed '{}'", e),
            }
        }

        runners
    }

    pub async fn parse_runner(path: PathBuf) -> Result<Runner, Box<dyn Error>> {
        use async_std::fs::read_to_string;
        let file = read_to_string(&path).await?;
        let mut channel: Runner = serde_json::from_str(&file)?;
        channel.location = path.parent().map(|x| x.into());
        Ok(channel)
    }
}
