use std::rc::Rc;

use anyhow::Result;
use clap::Args;
use tokio::task::JoinHandle;

use crate::{make_domain, text::Renderer};
use engine::{DevNullNotifier, Session};

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
    #[arg(short, long, default_value = "look")]
    text: Vec<String>,
    #[arg(short, long)]
    separate_sessions: bool,
}

fn evaluate_commands(domain: engine::Domain, cmd: Command) -> Result<()> {
    let renderer = Renderer::new()?;

    let mut open_session: Option<Rc<Session>> = None;
    for text in &cmd.text {
        open_session = match open_session {
            Some(session) => Some(session),
            None => Some(domain.open_session()?),
        };

        if let Some(reply) = open_session
            .as_ref()
            .expect("No open session")
            .evaluate_and_perform(&cmd.username, text)?
        {
            let text = renderer.render_reply(&reply)?;
            println!("{}", text);
        }

        if cmd.separate_sessions {
            if let Some(session) = open_session.take() {
                session.close(&DevNullNotifier::default())?;
            }
        }
    }

    if let Some(session) = open_session.take() {
        session.close(&DevNullNotifier::default())?;
    }

    Ok(())
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain().await?;
    let cmd = cmd.clone();

    let handle: JoinHandle<Result<()>> =
        tokio::task::spawn_blocking(|| evaluate_commands(domain, cmd));

    handle.await?
}
