use std::collections::HashMap;

use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::channel::ChannelConfig;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StepArg {
    pub id:          String,
    #[serde(rename = "type")]
    pub ty:          String,
    #[serde(flatten)]
    pub other:       Map<String, Value>,
    #[serde(default)]
    pub default:     bool,
    #[serde(default)]
    pub value:       String,
    #[serde(default)]
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Step {
    pub id:        String,
    #[serde(rename = "runnerId")]
    pub runner_id: String,
    pub config:    Value,
    pub build:     Option<String>,
    pub args:      Vec<StepArg>,
    pub location:  Option<String>,
}

pub fn config_is_valid(schema: &JSONSchema, config: &Value) -> bool {
    if let Err(e) = schema.validate(config) {
        e.into_iter().for_each(|e| {
            eprintln!("Steps is not valid according to runner! {}", e);
        });
        return false;
    }

    true
}

#[cfg(feature = "io")]
pub use io::*;

#[cfg(feature = "io")]
mod io {
    use std::collections::HashMap;
    use std::error::Error;
    use std::path::Path;

    use super::*;
    use crate::runner::Runner;

    pub async fn parse_steps<'a, S, I>(
        paths: I,
        runners: &'a [Runner],
    ) -> Vec<Step>
    where
        S: AsRef<Path> + 'a,
        I: IntoIterator<Item = &'a S>,
    {
        let mut steps = Vec::new();
        let iterator = paths.into_iter().map(parse_step);

        let mut per_id = HashMap::<String, u32>::new();

        for item in iterator {
            match item.await {
                Ok(mut step) => {
                    if let Some(runner) = runners
                        .iter()
                        .find(|runner| runner.id == step.runner_id)
                    {
                        if config_is_valid(&runner.schema, &step.config) {
                            let number = if let Some(n) = per_id.get(&step.id) {
                                n + 1
                            } else {
                                1
                            };

                            per_id.insert(step.id.to_string(), number);
                            step.id = format!("{}_{}", step.id, number);

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
        use async_std::fs::read_to_string;

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Output {
    Stdout,
    Stderr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RunThing {
    #[serde(rename = "processorConfig")]
    pub processor_config: Step,
    pub(crate) args:      HashMap<String, StepArgument>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubStep {
    pub run:           RunThing,
    pub serialization: String,
    pub output:        Output,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StepArgument {
    StreamReader {
        fields: HashMap<String, ChannelConfig>,
    },
    StreamWriter {
        fields: HashMap<String, ChannelConfig>,
    },
    File {
        path:          String,
        serialization: String,
    },
    Plain {
        value: Value,
    },
    Step {
        #[serde(flatten)]
        sub: SubStep,
    },
    Param {
        name: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StepArguments {
    pub step:  Step,
    arguments: HashMap<String, StepArgument>,
}

impl StepArguments {
    pub fn new(from: &Step) -> Self {
        Self {
            step:      from.clone(),
            arguments: HashMap::new(),
        }
    }

    pub fn add_argument(&mut self, id: String, value: StepArgument) {
        self.arguments.insert(id, value);
    }

    pub fn use_target(&mut self, id: &str, field: &str, config: ChannelConfig) {
        if !self.arguments.contains_key(id) {
            self.arguments.insert(
                id.to_string(),
                StepArgument::StreamWriter {
                    fields: HashMap::new(),
                },
            );
        }

        match self.arguments.get_mut(id) {
            Some(StepArgument::StreamWriter { ref mut fields }) => {
                fields.insert(field.to_string(), config);
            }
            _ => panic!("expected a stream writer"),
        }
    }

    pub fn into_runthing(self) -> RunThing {
        RunThing {
            processor_config: self.step,
            args:             self.arguments,
        }
    }

    pub fn into_value(self) -> RunThing {
        let mut out = HashMap::new();

        self.arguments.into_iter().for_each(|(id, arg)| {
            out.insert(id, arg);
        });

        RunThing {
            processor_config: self.step,
            args:             out,
        }
    }
}
