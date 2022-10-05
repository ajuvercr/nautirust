use std::collections::{HashMap, HashSet};
use std::fmt::Display;

use dialoguer::{Completion, Input};
use serde_json::{Map, Value};

use super::command::Runtime;
use super::user;
use crate::channel::ChannelConfig;
use crate::commands::run::Steps;
use crate::step::{
    Output, Step, StepArg, StepArgument, StepArguments, SubStep,
};

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

#[derive(Debug)]
pub struct TmpTarget<'a> {
    pub step_id:                 &'a str,
    pub writer_id:               &'a str,
    pub name:                    &'a str,
    pub possible_channels:       &'a Vec<String>,
    pub possible_serializations: &'a Vec<String>,
}

impl<'a> Display for TmpTarget<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.step_id, self.writer_id, self.name)
    }
}

#[derive(Default)]
pub struct State<'a> {
    open_channels: Vec<TmpTarget<'a>>,
    all_step_args: HashMap<String, StepArguments>,
    done:          Vec<String>,
    used:          HashSet<String>,
    params: Vec<String>,
}

pub struct Ctx<'a> {
    channels:       &'a Vec<String>,
    serializations: &'a Vec<String>,
}

impl<'a> State<'a> {
    pub fn apply_step(
        &mut self,
        automatic: bool,
        step: &'a Step,
        runtime: &mut Runtime<'a>,
    ) {
        let ctx = Ctx {
            channels:       runtime.channels.get(&step.runner_id).unwrap(),
            serializations: runtime
                .serializations
                .get(&step.runner_id)
                .unwrap(),
        };

        let mut step_args = StepArguments::new(step);

        println!("Chapter: {}", runtime.style.chapter.apply_to(&step.id));

        for arg in &step.args {
            match arg.ty.as_str() {
                "streamReader" => {
                    let (id, arg) =
                        self.apply_reader_arg(arg, automatic, &ctx, runtime);
                    step_args.add_argument(id, arg);
                }
                "streamWriter" => self.apply_writer_arg(step, arg, &ctx),
                _ => {
                    let (id, arg) = self.apply_normal_arg(arg, &ctx, runtime);
                    step_args.add_argument(id, arg);
                }
            }
        }

        if self
            .all_step_args
            .insert(step.id.to_string(), step_args)
            .is_some()
        {
            panic!("Found multiple steps with the same id '{}'", step.id);
        }

        self.done.push(step.id.to_string());
    }

    fn apply_reader_arg(
        &mut self,
        arg: &StepArg,
        automatic: bool,
        ctx: &Ctx,
        runtime: &mut Runtime,
    ) -> (String, StepArgument) {
        let style = &runtime.style;
        println!("Set up stream reader {}", style.arg.apply_to(&arg.id));
        if !arg.description.is_empty() {
            println!("Description: {}", style.arg.apply_to(&arg.description),);
        }
        let source_ids =
            extract_string_array(&arg.other, "sourceIds").unwrap_or_default();

        let mut fields = HashMap::new();

        for id in &source_ids {
            // todo! make better
            let (config, tmp_target) = user::ask_channel_config(
                id,
                ctx.channels,
                ctx.serializations,
                &mut self.open_channels,
                &mut runtime.channel_options,
                automatic,
            )
            .expect("no good thing found");

            if let Some(tmp_target) = tmp_target {
                self.all_step_args
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
        (arg.id.clone(), argument)
    }

    fn apply_writer_arg(
        &mut self,
        step: &'a Step,
        arg: &'a StepArg,
        ctx: &Ctx<'a>,
    ) {
        let ids =
            extract_string_array(&arg.other, "targetIds").unwrap_or_default();

        for id in ids {
            let target = TmpTarget {
                name:                    id,
                writer_id:               &arg.id,
                step_id:                 &step.id,
                possible_channels:       ctx.channels,
                possible_serializations: ctx.serializations,
            };
            self.open_channels.push(target);
        }
    }

    fn apply_normal_arg(
        &mut self,
        arg: &StepArg,
        ctx: &Ctx,
        runtime: &Runtime,
    ) -> (String, StepArgument) {
        let style = &runtime.style;
        println!(
            "Argument: {} ({})",
            style.arg.apply_to(&arg.id),
            style.ty.apply_to(&arg.ty)
        );

        if !arg.description.is_empty() {
            println!("Description: {}", style.arg.apply_to(&arg.description),);
        }

        let input_options = ["plain", "file", "process", "param"];

        let input_choice =
            user::ask_user_for("input type", &input_options, false);

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
                    user::ask_until_ready(|| prompt.interact_text())
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
                let path = user::ask_until_ready(|| prompt.interact_text());

                let serialization =
                    user::ask_user_for_serialization(ctx.serializations);

                StepArgument::File {
                    path,
                    serialization,
                }
            }
            "process" => self.process_output(runtime, ctx),
            "param" => {
                let mut prompt = Input::<String>::new();
                prompt.with_prompt("Name: ");

                let name = user::ask_until_ready(|| prompt.interact_text());
                self.params.push(name.clone());

                StepArgument::Param { name }
            }
            _ => unreachable!(),
        };

        (arg.id.to_string(), argument)
    }

    fn process_output(&mut self, runtime: &Runtime, ctx: &Ctx) -> StepArgument {
        let process_index =
            user::ask_user_for("Process Name", &self.done, false);
        let output =
            user::ask_user_for("Process output", &["stdout", "stderr"], false);
        let output = if output == 0 {
            Output::Stdout
        } else {
            Output::Stderr
        };
        self.used.insert(self.done[process_index].to_string());
        let linked_step =
            self.all_step_args.get(&self.done[process_index]).unwrap();
        let linked_step_ser =
            runtime.serializations[&linked_step.step.runner_id];
        let possible_sers: Vec<_> = ctx
            .serializations
            .iter()
            .filter(|x| linked_step_ser.iter().any(|y| x == &y))
            .collect();
        let serialization = user::ask_user_for_serialization(&possible_sers);
        StepArgument::Step {
            sub: SubStep {
                run: linked_step.clone().into_runthing(),
                output,
                serialization,
            },
        }
    }

    pub fn complete(mut self, automatic: bool, runtime: &mut Runtime) -> Steps {
        if !self.open_channels.is_empty() {
            println!("Lingering channels detected!");
            println!("Use remaining channel");

            for target in self.open_channels {
                println!(
                    "for {}.{}.{}",
                    target.step_id, target.writer_id, target.name
                );

                let (config, ty) = user::ask_user_for_channel(
                    target.possible_channels,
                    &mut runtime.channel_options,
                    automatic,
                );

                let ser = user::ask_user_for_serialization(
                    target.possible_serializations,
                );

                let ch_config = ChannelConfig::new(ty.to_string(), ser, config);

                self.all_step_args
                    .get_mut(target.step_id)
                    .unwrap()
                    .use_target(target.writer_id, target.name, ch_config);
            }
        }

        let args = self
            .all_step_args
            .into_values()
            .filter(|args| !self.used.contains(&args.step.id))
            .map(StepArguments::into_value)
            .collect::<Vec<_>>();

        println!("Got {} steps", args.len());

        Steps {
            steps:  args,
            params: self.params,
        }
    }
}
