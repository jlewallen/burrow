use anyhow::Result;
use chrono::Utc;
use clap::Args;
use std::{ops::Sub, rc::Rc};
use tracing::info;

use crate::{text::Renderer, DomainBuilder};
use engine::{AfterTick, DevNullNotifier, Domain, Session, SessionOpener};
use kernel::prelude::Effect;

#[derive(Debug, Args, Clone)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
    #[arg(short, long)]
    text: Vec<String>,
    #[arg(short, long)]
    separate_sessions: bool,
    #[arg(short, long)]
    deliver: bool,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
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

        if let Some(effect) = open_session
            .as_ref()
            .expect("No open session")
            .evaluate_and_perform(&cmd.username, text)?
        {
            match effect {
                Effect::Reply(reply) => match reply {
                    kernel::prelude::EffectReply::TaggedJson(tagged) => {
                        let text = renderer.render_value(&tagged.into_tagged())?;
                        println!("{}", text);
                    }
                },
                _ => todo!(),
            }
        }

        if cmd.separate_sessions {
            if let Some(session) = open_session.take() {
                session.close(&DevNullNotifier::default())?;
            }
        }
    }

    let notifier = DevNullNotifier::default();

    if let Some(session) = open_session.take() {
        session.close(&notifier)?;
    }

    if cmd.deliver {
        let now = Utc::now();
        match domain.tick(now, &notifier)? {
            AfterTick::Deadline(deadline) => {
                let delay = deadline.sub(now).num_milliseconds();
                info!(%deadline, delay_ms = delay, "deadline")
            }
            AfterTick::Processed(processed) => info!(%processed, "delivered"),
            AfterTick::Empty => {}
        }
    }

    domain.stop()?;

    Ok(())
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();
    let domain = builder.build().await?;
    let cmd = cmd.clone();

    tokio::task::spawn_blocking(|| evaluate_commands(domain, cmd)).await?
}
