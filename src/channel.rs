use std::error::Error;
use std::path::PathBuf;

use async_std::fs::read_to_string;
use glob::glob;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
pub struct Channel {
    pub id:              String,
    #[serde(rename = "requiredFields")]
    pub required_fields: Vec<String>,
    #[serde(skip_serializing)]
    pub schema:          JSONSchema,
}

impl<'de> Deserialize<'de> for Channel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Ch {
            id:              String,
            #[serde(rename = "requiredFields")]
            required_fields: Vec<String>,
        }
        let Ch {
            required_fields,
            id,
        } = <Ch as Deserialize>::deserialize(deserializer)?;

        let schema = json!({
            "type": "object",
            "required": required_fields
        });

        let schema = JSONSchema::compile(&schema).expect("valid schema");

        Ok(Channel {
            id,
            schema,
            required_fields,
        })
    }
}

pub async fn parse_channels(path: &str) -> Vec<Channel> {
    let mut channels = Vec::new();
    let mut iterator = glob(path)
        .expect("Failed to read channels glob pattern")
        .flatten()
        .map(parse_channel);

    while let Some(item) = iterator.next() {
        match item.await {
            Ok(channel) => channels.push(channel),
            Err(e) => eprintln!("Parsing channel failed '{}'", e),
        }
    }

    channels
}

pub async fn parse_channel(path: PathBuf) -> Result<Channel, Box<dyn Error>> {
    let file = read_to_string(path).await?;
    let channel: Channel = serde_json::from_str(&file)?;
    Ok(channel)
}
