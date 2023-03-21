use anyhow::Result;
use clap::Args;

use crate::{make_domain, text::Renderer};
use engine::DevNullNotifier;

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
    #[arg(short, long, default_value = "look")]
    text: String,
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain()?;
    let session = domain.open_session()?;
    let renderer = Renderer::new()?;

    for text in &[&cmd.text] {
        if let Some(reply) = session.evaluate_and_perform(&cmd.username, text)? {
            let text = renderer.render(reply)?;
            println!("{}", text);
        }
    }

    session.close(&DevNullNotifier::default())?;

    Ok(())
}
