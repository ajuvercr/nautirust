use std::collections::HashMap;

use async_std::fs;
use dialoguer::console::Style;
use serde_json::Value;

use crate::channel::Channel;
use crate::commands::generate::state::State;
use crate::runner::Runner;
use crate::step;

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

pub struct Styles {
    pub chapter: Style,
    pub arg:     Style,
    pub ty:      Style,
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            chapter: Style::new().bold().bright(),
            arg:     Style::new().underlined().bright(),
            ty:      Style::new().italic(),
        }
    }
}

pub struct Runtime<'a> {
    pub style:           Styles,
    pub channels:        HashMap<String, &'a Vec<String>>,
    pub serializations:  HashMap<String, &'a Vec<String>>,
    pub channel_options: HashMap<String, Vec<Value>>,
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
        let channel_options = channels
            .iter()
            .map(|ch| (ch.id.clone(), ch.options.clone()))
            .collect::<HashMap<_, _>>();

        let mut runtime = Runtime {
            style: Styles::default(),
            channels: channels_per_runner,
            serializations: serializations_per_runner,
            channel_options,
        };

        let mut state = State::default();

        for step in &steps {
            state.apply_step(self.automatic, step, &mut runtime);
        }

        let pretty = serde_json::to_string_pretty(
            &state.complete(self.automatic, &mut runtime),
        )
        .unwrap();

        if let Some(location) = self.output {
            fs::write(location, pretty.as_bytes()).await.unwrap();
        } else {
            println!("\n");
            println!("{}", pretty);
        }
    }
}
