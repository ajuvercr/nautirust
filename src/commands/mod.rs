use clap::Subcommand;

use crate::channel::Channel;
use crate::runner::Runner;

pub mod generate;
pub mod run;
pub mod validate;

#[derive(Subcommand, Debug)]
pub enum Command {
    Generate(generate::Command),
    Run(run::Command),
    Validate(validate::Command),
}

impl Command {
    pub async fn execute(self, channels: Vec<Channel>, runners: Vec<Runner>) {
        match self {
            Command::Generate(gen) => gen.execute(channels, runners).await,
            Command::Run(run) => run.execute(channels, runners).await,
            Command::Validate(validate) => {
                validate.execute(channels, runners).await
            }
        }
    }
}
