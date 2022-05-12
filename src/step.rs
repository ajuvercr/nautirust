use std::error::Error;

use async_std::fs::read_to_string;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::runner::Runner;

#[derive(Serialize, Deserialize, Debug)]
pub struct Step {
    id:        String,
    #[serde(rename = "runnerId")]
    runner_id: String,
    config:    Value,
    args:      Vec<Value>,
}

fn config_is_valid(schema: &JSONSchema, config: &Value) -> bool {
    if let Err(e) = schema.validate(config) {
        e.into_iter().for_each(|e| {
            eprintln!("Steps is not valid according to runner! {}", e);
        });
        return false;
    }
    return true;
}

pub async fn parse_steps<'a, S, I>(
    paths: I,
    runners: &'a Vec<Runner>,
) -> Vec<Step>
where
    S: AsRef<str> + 'a,
    I: IntoIterator<Item = &'a S>,
{
    let mut steps = Vec::new();
    let mut iterator = paths.into_iter().map(parse_step);

    while let Some(item) = iterator.next() {
        match item.await {
            Ok(step) => {
                if let Some(runner) =
                    runners.iter().find(|runner| &runner.id == &step.runner_id)
                {
                    if config_is_valid(&runner.schema, &step.config) {
                        steps.push(step);
                    }
                } else {
                    eprintln!("No runner found for id {}", step.runner_id);
                }
            }
            Err(e) => eprintln!("Parsing step failed '{}'", e),
        }
    }

    steps
}

pub async fn parse_step<'a, S: AsRef<str>>(
    path: &'a S,
) -> Result<Step, Box<dyn Error>> {
    let file = read_to_string(path.as_ref()).await?;
    let channel: Step = serde_json::from_str(&file)?;
    Ok(channel)
}
