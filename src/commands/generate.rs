use std::collections::HashMap;
use std::fmt::Display;
use std::io::{self, BufRead};

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
    step_id:                 &'a str,
    writer_id:               &'a str,
    name:                    &'a str,
    possible_channels:       &'a Vec<String>,
    possible_serializations: &'a Vec<String>,
}

fn read_std_line() -> String {
    let stdin = io::stdin();
    let mut iterator = stdin.lock().lines();
    let out = iterator.next().unwrap().unwrap();
    println!("got line {}", out);
    out
}

impl<'a> Display for TmpTarget<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.step_id, self.writer_id, self.name)
    }
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
    pub(crate) async fn execute<'a>(
        self,
        channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let steps = step::parse_steps(&self.steps, &runners).await;

        let channels_per_runner: HashMap<String, &'_ Vec<String>> = runners
            .iter()
            .map(|r| (r.id.clone(), &r.can_use_channel))
            .collect();

        let serializations_per_runner: HashMap<String, &'_ Vec<String>> =
            runners
                .iter()
                .map(|r| (r.id.clone(), &r.can_use_serialization))
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

            let serialization_types =
                serializations_per_runner.get(&step.runner_id).unwrap();

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
                                serialization_types,
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
                                name:                    id,
                                writer_id:               &arg.id,
                                step_id:                 &step.id,
                                possible_channels:       channel_types,
                                possible_serializations: serialization_types,
                            };
                            open_channels.push(target);
                        }
                    }
                    _ => {
                        let value = loop {
                            println!("{}: type {}", arg.id, arg.ty);

                            let inp: String = read!("{}\n");
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
                println!(
                    "for {}.{}.{}",
                    target.step_id, target.writer_id, target.name
                );

                let (config, ty) = ask_user_for_channel(
                    &target.possible_channels,
                    &mut channel_options,
                );
                let ser =
                    ask_user_for_serialization(&target.possible_serializations);

                let ch_config = ChannelConfig::new(
                    target.name.to_string(),
                    ty.to_string(),
                    ser,
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

fn create_valid_tmp_target_fn<'a>(
    channel_types: &'a Vec<String>,
    ser_types: &'a Vec<String>,
) -> impl for<'r, 's> Fn(&'r TmpTarget<'s>) -> bool + 'a {
    |ch: &TmpTarget| {
        ch.possible_channels
            .iter()
            .any(|c| channel_types.contains(c))
            && ch
                .possible_serializations
                .iter()
                .any(|c| ser_types.contains(c))
    }
}

fn ask_channel_config<'a>(
    id: &str,
    channel_types: &Vec<String>,
    ser_types: &Vec<String>,
    open_channels: &mut Vec<TmpTarget<'a>>,
    channel_options: &mut HashMap<String, Vec<Value>>,
) -> Option<(ChannelConfig, Option<TmpTarget<'a>>)> {
    let is_valid_tmp_target =
        create_valid_tmp_target_fn(channel_types, ser_types);

    let options = open_channels
        .iter()
        .filter(|&x| is_valid_tmp_target(x))
        .collect::<Vec<_>>();

    let n = ask_user_for(
        &format!("'{}' wants a channel, options:", id),
        &options,
        true,
    );

    let (target, types, sers) = {
        if n >= options.len() {
            (None, channel_types.clone(), ser_types.clone())
        } else {
            let target = open_channels.remove(n);

            let types: Vec<String> = target
                .possible_channels
                .iter()
                .filter(|c| channel_types.contains(c))
                .cloned()
                .collect();

            let sers: Vec<String> = target
                .possible_serializations
                .iter()
                .filter(|c| ser_types.contains(c))
                .cloned()
                .collect();

            (Some(target), types, sers)
        }
    };

    let (config, ty) = ask_user_for_channel(&types, channel_options);
    let ser = ask_user_for_serialization(&sers);

    Some((
        ChannelConfig::new(id.to_string(), ty.to_string(), ser, config),
        target,
    ))
}

fn ask_user_for_serialization(options: &Vec<String>) -> String {
    let ser_index = ask_user_for("What serialization?", &options, false);

    options[ser_index].to_string()
}

fn ask_user_for_channel<'a>(
    types: &'a Vec<String>,
    channel_options: &mut HashMap<String, Vec<Value>>,
) -> (Value, &'a String) {
    let ty_index = ask_user_for("Choose channel type", &types, false);
    let ty = &types[ty_index];

    let options = channel_options.get_mut(ty).unwrap();

    let channel_index = ask_user_for("Choose channel config", options, false);

    (options.remove(channel_index), ty)
}

fn ask_user_for<'a, T: std::fmt::Display>(
    name: &str,
    things: &'a Vec<T>,
    allow_other: bool,
) -> usize {
    println!("{}", name);

    things
        .iter()
        .enumerate()
        .for_each(|(i, t)| println!("{}: {}", i, t));

    if allow_other {
        println!("{}: Other", things.len());
    }

    let index: usize = read!("{}");
    println!();

    if index > things.len() || (!allow_other && index == things.len()) {
        return ask_user_for(name, things, allow_other);
    }

    index
}
