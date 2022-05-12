use crate::channel::Channel;
use crate::runner::Runner;

/// Validate configs for runners and channels
#[derive(clap::Args, Debug)]
pub struct Command {}
impl Command {
    pub async fn execute(
        &self,
        _channels: Vec<Channel>,
        _runners: Vec<Runner>,
    ) {
        // things are valid
    }
}
