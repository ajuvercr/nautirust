use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use async_std::fs;
use dialoguer::console::Style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Completion, FuzzySelect, Input};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::channel::{Channel, ChannelConfig};
use crate::runner::Runner;
use crate::step::{self, Output, Step, StepArgument, StepArguments, SubStep};

/// Generate a pipeline of steps
#[derive(clap::Args, Debug)]
pub struct Command {
    /// Steps to include in the pipeline (ordered)
    steps: Vec<String>,

    /// Output location of the generated pipeline file
    #[clap(short, long)]
    output: Option<String>,

    /// Try infer basic configurations details
    #[clap(short, long)]
    automatic: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RunConfig {
    #[serde(rename = "processorConfig")]
    processor: Step,
    args:      HashMap<String, StepArgument>,
}

#[derive(Debug)]
struct TmpTarget<'a> {
    step_id:                 &'a str,
    writer_id:               &'a str,
    name:                    &'a str,
    possible_channels:       &'a Vec<String>,
    possible_serializations: &'a Vec<String>,
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

struct Complete;
impl Completion for Complete {
    fn get(&self, input: &str) -> Option<String> {
        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("compgen -f {}", input))
            .output()
            .expect("failed to execute compgen");
        let out_str = String::from_utf8_lossy(&output.stdout);

        let mut lines = out_str.lines();
        let mut common = lines.next()?;

        for line in lines {
            let mut index = 0;

            for (i, j) in line.chars().zip(common.chars()) {
                if i != j {
                    break;
                }

                index += 1;
            }

            common = &common[..index];
        }

        if common.is_empty() {
            None
        } else {
            Some(common.to_string())
        }
    }
}

impl Command {
    pub(crate) async fn execute<'a>(
        self,
        channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let chapter_style = Style::new().bold().bright();
        let arg_style = Style::new().underlined().bright();
        let type_style = Style::new().italic();

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
        let mut done_steps = Vec::<String>::new();
        let mut used_steps = HashSet::<String>::new();

        for step in &steps {
            let mut step_args = StepArguments::new(step);

            let channel_types =
                channels_per_runner.get(&step.runner_id).unwrap();

            let serialization_types =
                serializations_per_runner.get(&step.runner_id).unwrap();

            println!("Chapter: {}", chapter_style.apply_to(&step.id));

            for arg in &step.args {
                match arg.ty.as_str() {
                    "streamReader" => {
                        println!(
                            "Set up stream reader {}",
                            arg_style.apply_to(&arg.id)
                        );
                        if !arg.description.is_empty() {
                            println!(
                                "Description: {}",
                                arg_style.apply_to(&arg.description),
                            );
                        }
                        let source_ids =
                            extract_string_array(&arg.other, "sourceIds")
                                .unwrap_or_default();

                        let mut fields = HashMap::new();

                        for id in &source_ids {
                            // todo! make better
                            let (config, tmp_target) = ask_channel_config(
                                id,
                                channel_types,
                                serialization_types,
                                &mut open_channels,
                                &mut channel_options,
                                self.automatic,
                            )
                            .expect("no good thing found");

                            if let Some(tmp_target) = tmp_target {
                                all_step_args
                                    .get_mut(tmp_target.step_id)
                                    .unwrap()
                                    .use_target(
                                        tmp_target.writer_id,
                                        tmp_target.name,
                                        config.clone(),
                                    );
                            }

                            fields.insert(id.to_string(), config);
                        }

                        let argument = StepArgument::StreamReader { fields };
                        step_args.add_argument(arg.id.to_string(), argument);
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
                        println!(
                            "Argument: {} ({})",
                            arg_style.apply_to(&arg.id),
                            type_style.apply_to(&arg.ty)
                        );
                        if !arg.description.is_empty() {
                            println!(
                                "Description: {}",
                                arg_style.apply_to(&arg.description),
                            );
                        }
                        let input_options = ["plain", "file", "process"];

                        let input_choice =
                            ask_user_for("input type", &input_options, false);

                        let argument = match input_options[input_choice] {
                            "plain" => {
                                let string = if arg.default {
                                    arg.value.clone()
                                } else {
                                    let mut prompt = Input::<String>::new();
                                    prompt
                                        .with_prompt(" ")
                                        .with_initial_text(arg.value.clone())
                                        .completion_with(&Complete);
                                    ask_until_ready(|| prompt.interact_text())
                                };

                                let value = serde_json::to_value(&string)
                                    .unwrap_or(Value::String(string));
                                StepArgument::Plain { value }
                            }
                            "file" => {
                                let mut prompt = Input::<String>::new();
                                prompt
                                    .with_prompt("Path: ")
                                    .with_initial_text(arg.value.clone())
                                    .completion_with(&Complete);
                                let path =
                                    ask_until_ready(|| prompt.interact_text());

                                let serialization = ask_user_for_serialization(
                                    serialization_types,
                                );

                                StepArgument::File {
                                    path,
                                    serialization,
                                }
                            }
                            "process" => {
                                let process_index = ask_user_for(
                                    "Process Name",
                                    &done_steps,
                                    false,
                                );
                                let output = ask_user_for(
                                    "Process output",
                                    &["stdout", "stderr"],
                                    false,
                                );
                                let output = if output == 0 {
                                    Output::Stdout
                                } else {
                                    Output::Stderr
                                };

                                used_steps.insert(
                                    done_steps[process_index].to_string(),
                                );
                                let linked_step = all_step_args
                                    .get(&done_steps[process_index])
                                    .unwrap();
                                let linked_step_ser = serializations_per_runner
                                    [&linked_step.step.runner_id];

                                let possible_sers: Vec<_> = serialization_types
                                    .iter()
                                    .filter(|x| {
                                        linked_step_ser.iter().any(|y| x == &y)
                                    })
                                    .collect();
                                let serialization =
                                    ask_user_for_serialization(&possible_sers);

                                StepArgument::Step {
                                    sub: SubStep {
                                        run: linked_step
                                            .clone()
                                            .into_runthing(),
                                        output,
                                        serialization,
                                    },
                                }
                            }
                            _ => unreachable!(),
                        };

                        step_args.add_argument(arg.id.to_string(), argument);
                    }
                }
            }

            if all_step_args
                .insert(step.id.to_string(), step_args)
                .is_some()
            {
                panic!("Found multiple steps with the same id '{}'", step.id);
            }

            done_steps.push(step.id.to_string());
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
                    target.possible_channels,
                    &mut channel_options,
                    self.automatic,
                );

                let ser =
                    ask_user_for_serialization(target.possible_serializations);

                let ch_config = ChannelConfig::new(ty.to_string(), ser, config);

                all_step_args.get_mut(target.step_id).unwrap().use_target(
                    target.writer_id,
                    target.name,
                    ch_config,
                );
            }
        }

        let args = all_step_args
            .into_values()
            .filter(|args| !used_steps.contains(&args.step.id))
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
    channel_types: &'a [String],
    ser_types: &'a [String],
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

fn get_if_only_one<T, I: Iterator<Item = T>>(mut iter: I) -> Option<T> {
    iter.next()
        .and_then(|v| if iter.next().is_some() { None } else { Some(v) })
}

fn ask_channel_config<'a>(
    id: &str,
    channel_types: &[String],
    ser_types: &[String],
    open_channels: &mut Vec<TmpTarget<'a>>,
    channel_options: &mut HashMap<String, Vec<Value>>,
    automatic: bool,
) -> Option<(ChannelConfig, Option<TmpTarget<'a>>)> {
    let is_valid_tmp_target =
        create_valid_tmp_target_fn(channel_types, ser_types);

    let options = open_channels
        .iter()
        .filter(|&x| is_valid_tmp_target(x))
        .collect::<Vec<_>>();

    // Collect indicies of options with the same name
    let automatic_options =
        options.iter().enumerate().flat_map(|(index, option)| {
            if option.name == id {
                Some(index)
            } else {
                None
            }
        });

    let automatic_option = get_if_only_one(automatic_options);

    let n = if let (true, Some(n)) = (automatic, automatic_option) {
        let chapter_style = Style::new().bold().bright();
        println!("Linking with {}", chapter_style.apply_to(&options[n]));
        n
    } else {
        ask_user_for(
            &format!("'{}' wants a channel, options:", id),
            &options,
            true,
        )
    };

    // If a target is chosen (n < options.len()) then we need to determine the channel types and
    // serialization that are both possible for the current processor and that target
    let (target, types, sers) = {
        if n >= options.len() {
            (None, channel_types.to_owned(), ser_types.to_owned())
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

    let (config, ty) = ask_user_for_channel(&types, channel_options, automatic);
    let ser = ask_user_for_serialization(&sers);

    Some((ChannelConfig::new(ty.to_string(), ser, config), target))
}

fn ask_user_for_serialization<S: Display>(options: &[S]) -> String {
    let ser_index = ask_user_for("What serialization?", options, false);

    options[ser_index].to_string()
}

fn ask_user_for_channel<'a>(
    types: &'a [String],
    channel_options: &mut HashMap<String, Vec<Value>>,
    automatic: bool,
) -> (Value, &'a String) {
    let ty_index = ask_user_for("Choose channel type", types, false);
    let ty = &types[ty_index];

    let options = channel_options.get_mut(ty).unwrap();

    if automatic {
        let out = options.remove(0);
        let type_style = Style::new().italic();
        println!("Chosen channel config: {}", type_style.apply_to(&out));
        return (out, ty);
    }

    let channel_index = ask_user_for("Choose channel config", options, false);

    (options.remove(channel_index), ty)
}

fn ask_until_ready<T, E, F: FnMut() -> Result<T, E>>(mut f: F) -> T {
    loop {
        if let Ok(x) = f() {
            break x;
        }
    }
}

fn ask_user_for<T: std::fmt::Display>(
    name: &str,
    things: &'_ [T],
    allow_other: bool,
) -> usize {
    let theme = ColorfulTheme::default();
    let mut item = FuzzySelect::with_theme(&theme);

    item.items(things).with_prompt(name).default(0);

    if allow_other {
        item.item("Other");
    }

    loop {
        if let Ok(output) = item.interact() {
            break output;
        }
    }
}
