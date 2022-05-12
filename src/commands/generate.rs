use crate::channel::Channel;
use crate::runner::Runner;
use crate::step;

/// Generate json that is ready to execute
#[derive(clap::Args, Debug)]
pub struct Command {
    steps: Vec<String>,
}
impl Command {
    pub(crate) async fn execute(
        self,
        channels: Vec<Channel>,
        runners: Vec<Runner>,
    ) {
        let steps = step::parse_steps(&self.steps, &runners).await;

        todo!()
    }
}
