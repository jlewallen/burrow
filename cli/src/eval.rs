use std::rc::Rc;

use anyhow::Result;
use clap::Args;

use crate::{make_domain, text::Renderer, PluginConfiguration};
use engine::{DevNullNotifier, Domain, Session, SessionOpener};

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
    #[arg(short, long, default_value = "look")]
    text: Vec<String>,
    #[arg(short, long)]
    separate_sessions: bool,
}

impl Command {
    fn plugin_configuration(&self) -> PluginConfiguration {
        PluginConfiguration {
            wasm: false,
            dynlib: true,
            rune: false,
            rpc: false,
        }
    }
}

fn evaluate_commands(domain: Domain, cmd: Command) -> Result<()> {
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

    domain.stop()?;

    Ok(())
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain(cmd.plugin_configuration()).await?;
    let cmd = cmd.clone();

    tokio::task::spawn_blocking(|| evaluate_commands(domain, cmd)).await?
}
