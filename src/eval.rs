use anyhow::Result;
use clap::Args;

use crate::{
    domain::{DevNullNotifier, Domain},
    storage,
    text::Renderer,
};

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
    #[arg(short, long, default_value = "look")]
    text: String,
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let renderer = Renderer::new()?;
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = Domain::new(storage_factory, false);
    let session = domain.open_session()?;

    for text in &[&cmd.text] {
        if let Some(reply) = session.evaluate_and_perform(&cmd.username, text)? {
            let text = renderer.render(reply)?;
            println!("{}", text);
        }
    }

    session.close(&DevNullNotifier::default())?;

    Ok(())
}
