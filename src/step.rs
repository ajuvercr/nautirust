use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use async_std::fs::read_to_string;
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::channel::ChannelConfig;
use crate::runner::Runner;

#[derive(Serialize, Deserialize, Debug)]
pub struct StepArg {
    pub id:      String,
    #[serde(rename = "type")]
    pub ty:      String,
    #[serde(flatten)]
    pub other:   Map<String, Value>,
    #[serde(default = "default_bool")]
    pub default: bool,
    #[serde(default = "default_string")]
    pub value:   String,
}

fn default_string() -> String{
    String::default()
}

fn default_bool() -> bool{
    false
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Step {
    pub id:        String,
    #[serde(rename = "runnerId")]
    pub runner_id: String,
    pub config:    Value,
    pub build:     Option<String>,
    pub args:      Vec<StepArg>,
    pub location:  Option<String>,
}

fn config_is_valid(schema: &JSONSchema, config: &Value) -> bool {
    if let Err(e) = schema.validate(config) {
        e.into_iter().for_each(|e| {
            eprintln!("Steps is not valid according to runner! {}", e);
        });
        return false;
    }

    true
}

pub async fn parse_steps<'a, S, I>(paths: I, runners: &'a [Runner]) -> Vec<Step>
where
    S: AsRef<Path> + 'a,
    I: IntoIterator<Item = &'a S>,
{
    let mut steps = Vec::new();
    let iterator = paths.into_iter().map(parse_step);

    for item in iterator {
        match item.await {
            Ok(step) => {
                if let Some(runner) =
                    runners.iter().find(|runner| runner.id == step.runner_id)
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

pub async fn parse_step<S: AsRef<Path>>(
    path: &'_ S,
) -> Result<Step, Box<dyn Error>> {
    let p = path.as_ref();
    let loc = p
        .parent()
        .and_then(|x| x.canonicalize().ok())
        .map(|p| p.display().to_string());
    let file = read_to_string(path.as_ref()).await?;
    let mut channel: Step = serde_json::from_str(&file)?;
    channel.location = loc;
    Ok(channel)
}

pub struct StepArguments {
    step:          Value,
    stream_reader: HashMap<String, Vec<ChannelConfig>>,

    arguments: Vec<(String, Value)>,
}

impl StepArguments {
    pub fn new(step: &Step) -> Self {
        let value = serde_json::to_value(step).unwrap();
        Self {
            step:          value,
            stream_reader: HashMap::new(),
            arguments:     Vec::new(),
        }
    }

    pub fn add_argument(&mut self, id: String, value: Value) {
        self.arguments.push((id, value));
    }

    pub fn use_target(&mut self, id: &str, config: ChannelConfig) {
        if let Some(configs) = self.stream_reader.get_mut(id) {
            configs.push(config);
        } else {
            self.stream_reader.insert(id.to_string(), vec![config]);
        }
    }

    pub fn into_value(self) -> Value {
        let mut out = HashMap::new();

        self.arguments.into_iter().for_each(|(id, arg)| {
            out.insert(id, arg);
        });

        self.stream_reader.into_iter().for_each(|(id, reader)| {
            let value = serde_json::to_value(reader).unwrap();
            out.insert(id, value);
        });

        json!({
            "processorConfig": self.step,
            "args": out
        })
    }
}
