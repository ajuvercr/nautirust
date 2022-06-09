use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::channel::Channel;
use crate::runner::Runner;

pub mod generate;
pub mod prepare;
pub mod run;
pub mod stop;
pub mod validate;

#[derive(Subcommand, Debug)]
pub enum Command {
    Generate(generate::Command),
    Run(run::Command),
    Prepare(prepare::Command),
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
    dirs::home_dir().map(|mut h| {
        if h == Path::new("/") {
            // Corner case: `h` root directory;
            // don't prepend extra `/`, just drop the tilde.
            p.strip_prefix("~").unwrap().to_path_buf()
        } else {
            h.push(p.strip_prefix("~/").unwrap());
            h
        }
    })
}


fn start_subproc<S: AsRef<OsStr>>(
    script: &str,
    location: S,
) -> Vec<std::process::Child> {
    let command = shlex::split(&script).unwrap();

    start_subproc_cmdvec(expand_tilde(location.as_ref()).unwrap(), command)
}

fn start_subproc_cmdvec<S: AsRef<OsStr>>(
    location: S,
    command: Vec<String>,
) -> Vec<std::process::Child> {
    let location = Path::new(&location);

    let grouped_command = command.iter().fold( Vec::new(), |mut acc, x| {

        if x == "&&" || x == "||" {
            acc.push(Vec::new());
            return acc
        }
        if acc.is_empty(){
            acc.push(Vec::new());
        }
        acc.last_mut().unwrap().push(x); 
        acc
    });


    let mut proc_vec = Vec::new(); 

    for cmd  in grouped_command{
        let prog  = cmd[0]; 
        let args = &cmd[1..];
        let mut proc  = std::process::Command::new(prog); 
        
        proc.args(args);

        if location.exists() {
            proc.current_dir(location);
        }
        proc_vec.push(proc.spawn().unwrap());
    }


    proc_vec



}
