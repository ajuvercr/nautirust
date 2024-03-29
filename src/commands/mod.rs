use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::thread::{spawn, JoinHandle};

use clap::Subcommand;
use jsonpath_rust::JsonPathQuery;
use serde_json::Value;

use crate::channel::Channel;
use crate::runner::Runner;

pub mod docker;
pub mod generate;
pub mod prepare;
pub mod run;
pub mod stop;
pub mod validate;

#[derive(Subcommand, Debug)]
pub enum Command {
    Generate(generate::Command),
    Run(run::Command),
    Docker(docker::Command),
    Prepare(prepare::Command),
    Validate(validate::Command),
    Stop(stop::Command),
}

impl Command {
    pub async fn execute(self, channels: Vec<Channel>, runners: Vec<Runner>) {
        match self {
            Command::Generate(gen) => gen.execute(channels, runners).await,
            Command::Run(run) => run.execute(channels, runners).await,
            Command::Docker(docker) => docker.execute(channels, runners).await,
            Command::Validate(validate) => {
                validate.execute(channels, runners).await
            }
            Command::Stop(stop) => stop.execute(channels, runners).await,
            Command::Prepare(prepare) => {
                prepare.execute(channels, runners).await
            }
        }
    }
}

fn expand_tilde<P: AsRef<Path>>(path_user_input: P) -> Option<PathBuf> {
    let p = path_user_input.as_ref();
    if !p.starts_with("~") {
        return Some(p.to_path_buf());
    }
    if p == Path::new("~") {
        return dirs::home_dir();
    }

    let p = p.strip_prefix("~/").unwrap();

    dirs::home_dir().map(|mut h| {
        if h == Path::new("/") {
            // Corner case: `h` root directory;
            // don't prepend extra `/`, just drop the tilde.
            p.to_path_buf()
        } else {
            h.push(p);
            h
        }
    })
}

fn get_used_channels<'a>(
    content: &'a str,
    channels: &'a [Channel],
) -> impl Iterator<Item = &'a Channel> {
    let json: Value = serde_json::from_str(content).unwrap();
    let channel_types = json.path("$..type").unwrap();

    let array = if let Value::Array(a) = channel_types {
        a
    } else {
        // this is certain
        panic!();
    };

    let is_present = move |id: &str| array.iter().any(|ty| ty == id);
    channels.iter().filter(move |chan| is_present(&chan.id))
}

#[derive(Default)]
pub struct OutputConfig {
    stdout: bool,
    stderr: bool,
}

fn start_subproc<Str: AsRef<str>, S: AsRef<Path>>(
    script: Str,
    location: Option<S>,
    name: &str,
    output: OutputConfig,
) -> Option<(std::process::Child, JoinHandle<String>, JoinHandle<String>)> {
    let location = location.and_then(expand_tilde);

    let mut proc = std::process::Command::new("sh");
    proc.stdout(Stdio::piped());
    proc.stderr(Stdio::piped());
    proc.args(["-c", script.as_ref()]);

    if let Some(location) = location {
        proc.current_dir(location);
    }

    let mut child = proc.spawn().ok()?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let id1 = name.to_string();
    let h1 = spawn(move || {
        let mut lines = Vec::new();
        BufReader::new(stdout).lines().for_each(|line| {
            let line = line.unwrap_or_else(|_| String::from("error"));
            println!("\x1b[32mINFO\x1b[39m {}: {}", id1, line);
            if output.stdout {
                lines.push(line);
            }
        });
        lines.into_iter().collect()
    });

    let id2 = name.to_string();
    let h2 = spawn(move || {
        let mut lines = Vec::new();
        BufReader::new(stderr).lines().for_each(|line| {
            let line = line.unwrap_or_else(|_| String::from("error"));
            println!("\x1b[31mERRO\x1b[39m {}: {}", id2, line,);
            if output.stderr {
                lines.push(line);
            }
        });
        lines.into_iter().collect()
    });

    Some((child, h1, h2))
}

fn add_add_subproc<Str: AsRef<str>, S: AsRef<Path>>(
    script: &Option<Str>,
    location: Option<S>,
    procs: &mut Vec<(Child, JoinHandle<String>, JoinHandle<String>)>,
    id: &str,
    output: OutputConfig,
) {
    if let Some(stop_script) = script {
        let proc = start_subproc(stop_script, location, id, output);
        procs.extend(proc);
    }
}
