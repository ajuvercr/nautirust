use crate::channel::Channel;
use crate::runner::Runner;

/// Run the actual configs
#[derive(clap::Args, Debug)]
pub struct Command {
    file: Option<String>,
}
impl Command {
    pub(crate) async fn execute(self, channels: Vec<Channel>, runners: Vec<Runner>) {
        todo!()
    }
}
