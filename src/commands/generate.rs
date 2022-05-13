use std::collections::HashMap;

use async_std::fs;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::channel::{Channel, ChannelConfig};
use crate::runner::Runner;
use crate::step::{self, Step, StepArguments};

/// Generate json that is ready to execute
#[derive(clap::Args, Debug)]
pub struct Command {
    steps: Vec<String>,

    #[clap(short, long)]
    output: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunConfig {
    #[serde(rename = "processorConfig")]
    processor: Step,
    args:      HashMap<String, Value>,
}

#[derive(Debug)]
struct TmpTarget<'a> {
    step_id:           &'a str,
    writer_id:         &'a str,
    name:              &'a str,
    possible_channels: &'a Vec<String>,
}

fn extract_string_array<'a>(
    from: &'a Map<String, Value>,
    key: &str,
) -> Option<Vec<&'a str>> {
    let array = from.get(key)?.as_array()?;

    array
        .iter()
        .flat_map(Value::as_str)
        .collect::<Vec<_>>()
        .into()
}

impl Command {
    pub(crate) async fn execute(
        self,
        channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let steps = step::parse_steps(&self.steps, &runners).await;
        let pretty = serde_json::to_string_pretty(&steps).unwrap();
        println!("steps {}", pretty);

        let channels_per_runner: HashMap<String, Vec<String>> = runners
            .iter()
            .map(|r| (r.id.clone(), r.can_use_channel.clone()))
            .collect();

        // "kafka" => [{"topic": "epic"}, {"topic": "epic2"}]
        let mut channel_options = channels
            .iter()
            .map(|ch| (ch.id.clone(), ch.options.clone()))
            .collect::<HashMap<_, _>>();

        let mut open_channels: Vec<TmpTarget<'_>> = Vec::new();

        let mut all_step_args: HashMap<String, StepArguments> = HashMap::new();

        for step in &steps {
            let mut step_args = StepArguments::new(step);

            let channel_types =
                channels_per_runner.get(&step.runner_id).unwrap();

            println!("Getting arguments for '{}'", step.id);

            for arg in &step.args {
                match arg.ty.as_str() {
                    "streamReader" => {
                        println!("Stream Reader '{}'", arg.id);
                        let ids = extract_string_array(&arg.other, "sourceIds")
                            .unwrap_or_default();

                        let mut targets = Vec::new();

                        for id in &ids {
                            // todo! make better
                            let (config, tmp_target) = ask_channel_config(
                                id,
                                channel_types,
                                &mut open_channels,
                                &mut channel_options,
                            )
                            .expect("no good thing found");

                            if let Some(tmp_target) = tmp_target {
                                all_step_args
                                    .get_mut(tmp_target.step_id)
                                    .unwrap()
                                    .use_target(
                                        tmp_target.writer_id,
                                        config.with_name(tmp_target.name),
                                    );
                            }

                            targets.push(config);
                        }

                        let value = serde_json::to_value(targets).unwrap();
                        step_args.add_argument(arg.id.to_string(), value);
                    }
                    "streamWriter" => {
                        let ids = extract_string_array(&arg.other, "targetIds")
                            .unwrap_or_default();

                        for id in ids {
                            let target = TmpTarget {
                                name:              id,
                                writer_id:         &arg.id,
                                step_id:           &step.id,
                                possible_channels: channel_types,
                            };
                            open_channels.push(target);
                        }
                    }
                    _ => {
                        let value = loop {
                            println!("{}: type {}", arg.id, arg.ty);

                            let inp: String = read!();
                            println!();
                            if let Ok(v) = serde_json::from_str(&inp) {
                                break v;
                            }
                        };
                        step_args.add_argument(arg.id.to_string(), value);
                    }
                }
            }

            if let Some(_) =
                all_step_args.insert(step.id.to_string(), step_args)
            {
                panic!("Found multiple steps with the same id '{}'", step.id);
            }
        }

        if !open_channels.is_empty() {
            println!("Lingering channels detected!");
            println!("Use remaining channel");

            for target in open_channels {
                println!("for {}.{}.{}", target.step_id, target.writer_id, target.name);

                let (config, ty) = ask_user_for_channel(
                    &target.possible_channels,
                    &mut channel_options,
                );
                let ch_config = ChannelConfig::new(
                    target.name.to_string(),
                    ty.to_string(),
                    config,
                );

                all_step_args
                    .get_mut(target.step_id)
                    .unwrap()
                    .use_target(target.writer_id, ch_config);
            }
        }

        let args = all_step_args
            .into_values()
            .map(StepArguments::into_value)
            .collect::<Vec<_>>();

        println!("Got {} steps", args.len());

        let pretty =
            serde_json::to_string_pretty(&json!({ "values": args })).unwrap();

        if let Some(location) = self.output {
            fs::write(location, pretty.as_bytes()).await.unwrap();
        } else {
            println!("\n");
            println!("{}", pretty);
        }
    }
}

fn ask_channel_config<'a>(
    id: &str,
    channel_types: &Vec<String>,
    open_channels: &mut Vec<TmpTarget<'a>>,
    channel_options: &mut HashMap<String, Vec<Value>>,
) -> Option<(ChannelConfig, Option<TmpTarget<'a>>)> {
    println!("'{}' wants a channel, options:", id);
    let options = open_channels
        .iter()
        .filter(|ch| {
            ch.possible_channels
                .iter()
                .any(|c| channel_types.contains(c))
        })
        .collect::<Vec<_>>();

    options.iter().enumerate().for_each(|(i, target)| {
        println!(
            "{} {}.{}.{}",
            i, target.step_id, target.writer_id, target.name
        )
    });

    println!("{} Other source (not from previous steps)", options.len());

    let n: usize = read!();
    println!();

    let (target, types) = {
        if n >= options.len() {
            (None, channel_types.clone())
        } else {
            let target = open_channels.remove(n);

            let types: Vec<String> = target
                .possible_channels
                .iter()
                .filter(|c| channel_types.contains(c))
                .cloned()
                .collect();

            (Some(target), types)
        }
    };

    let (config, ty) = ask_user_for_channel(&types, channel_options);

    Some((
        ChannelConfig::new(id.to_string(), ty.to_string(), config),
        target,
    ))
}

fn ask_user_for_channel<'a>(
    types: &'a Vec<String>,
    channel_options: &mut HashMap<String, Vec<Value>>,
) -> (Value, &'a String) {
    println!("Choose channel type!");
    types
        .iter()
        .enumerate()
        .for_each(|(i, t)| println!("{}: {}", i, t));

    let ty: usize = read!();
    let ty = &types[ty];
    println!();

    let options = channel_options.get_mut(ty).unwrap();
    println!("Choose channel config!");

    options
        .iter()
        .enumerate()
        .for_each(|(i, t)| println!("{}: {}", i, t));

    let n: usize = read!();
    println!();

    (options.remove(n), ty)
}
