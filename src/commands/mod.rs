use std::ffi::OsStr;
use std::path::Path;

use clap::Subcommand;

use crate::channel::Channel;
use crate::runner::Runner;

pub mod generate;
pub mod run;
pub mod stop;
pub mod validate;

#[derive(Subcommand, Debug)]
pub enum Command {
    Generate(generate::Command),
    Run(run::Command),
    Validate(validate::Command),
    Stop(stop::Command),
}

impl Command {
    pub async fn execute(self, channels: Vec<Channel>, runners: Vec<Runner>) {
        match self {
            Command::Generate(gen) => gen.execute(channels, runners).await,
            Command::Run(run) => run.execute(channels, runners).await,
            Command::Validate(validate) => {
                validate.execute(channels, runners).await
            }
            Command::Stop(stop) => stop.execute(channels, runners).await,
        }
    }
}

fn start_subproc<S: AsRef<OsStr>>(
    script: &str,
    location: S,
) -> std::process::Child {
    let command = shlex::split(&script).unwrap();
    start_subproc_cmdvec(location, command)
}

fn start_subproc_cmdvec<S: AsRef<OsStr>>(
    location: S,
    command: Vec<String>,
) -> std::process::Child {
    let location = Path::new(&location);
    let first_cmd = &command[0];
    let snd_cmd = &command[1..];
    let mut proc = std::process::Command::new(first_cmd);
    proc.args(snd_cmd);
    if location.exists() {
        proc.current_dir(location);
    }
    proc.spawn().unwrap()
}
