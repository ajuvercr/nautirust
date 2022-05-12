use std::error::Error;

use async_std::path::Path;
use config::builder::DefaultState;
use config::ConfigBuilder;
use glob::glob;
use serde::{Deserialize, Serialize};

extern crate jsonschema;
#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate serde_json;
extern crate async_std;

use clap::{Parser, Subcommand};

mod channel;
mod runner;

const TOML_LOCATION: &'static str = "orchestrator.toml";

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Location of a config file
    #[clap(long)]
    config: Vec<String>,

    /// Glob to indicate channels locations
    #[clap(short, long)]
    channels: Option<String>,

    /// Glob to indicate runners locations
    #[clap(short, long)]
    runners: Option<String>,

    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Adds files to myapp
    Add { name: Option<String> },
    /// Run a steps
    Run {
        /// The actual steps
        steps: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, Debug)]
struct AppConfig {
    /// Glob to indicate channel locations
    channels: String,
    /// Glob to indicate runner locations
    runners:  String,

    /// Execute steps?
    steps: Option<Vec<String>>,
}

async fn load_cfg(args: Args) -> Result<AppConfig, Box<dyn Error>> {
    let mut tomls = args.config.clone();
    if tomls.is_empty() {
        tomls.push(TOML_LOCATION.to_string());
    }

    // First set some default value
    let mut builder = ConfigBuilder::<DefaultState>::default()
        .set_default("channels", "channels")?
        .set_default("runners", "runners")?;

    // Try to override with config things
    for toml in &tomls {
        if Path::new(&toml).exists().await {
            builder = builder.add_source(config::File::with_name(&toml));
        } else {
            eprintln!("config file not found '{}'", toml);
        }
    }

    // Try to override with environment values
    builder = builder.add_source(config::Environment::with_prefix("APP"));

    // Try to override with argument values
    builder = builder.set_override_option("channels", args.channels.clone())?;
    builder = builder.set_override_option("runners", args.runners.clone())?;

    match args.command {
        Some(Commands::Run { steps }) => {
            if !steps.is_empty() {
                builder = builder.set_override("steps", steps)?;
            } else {
                eprintln!("Cannot run without steps!");
            }
        }
        _ => {}
    }

    Ok(builder.build()?.try_deserialize()?)
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = load_cfg(Args::parse()).await?;
    println!("{:?}", config);

    let channels = channel::parse_channels(&config.channels).await;
    let runners = runner::parse_runners(&config.runners, &channels).await;

    let pretty = serde_json::to_string_pretty(&channels)?;
    println!("channels {}", pretty);

    let pretty = serde_json::to_string_pretty(&runners)?;
    println!("runners {}", pretty);

    Ok(())
}
