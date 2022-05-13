use crate::channel::Channel;
use crate::runner::Runner;

/// Validate configs for runners and channels
#[derive(clap::Args, Debug)]
pub struct Command {}
impl Command {
    pub async fn execute(&self, channels: Vec<Channel>, runners: Vec<Runner>) {
        let pretty = serde_json::to_string_pretty(&channels).unwrap();
        println!("channels\n{}", pretty);
        // things are valid
        let pretty = serde_json::to_string_pretty(&runners).unwrap();
        println!("runners\n{}", pretty);
    }
}
