use anyhow::{Context, Result};
use clap::Args;
use plugins_rune::RUNE_EXTENSION;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::task::JoinHandle;

use crate::terminal::Renderer;
use crate::PluginConfiguration;
use crate::{make_domain, terminal::default_external_editor};
use engine::{self, DevNullNotifier, Domain, Notifier, Session, SessionOpener};
use kernel::{ActiveSession, EntityKey, Perform, Reply, SimpleReply};
use replies::EditorReply;

use plugins_core::building::actions::SaveWorkingCopyAction;
use plugins_rune::actions::SaveScriptAction;

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
}

impl Command {
    fn plugin_configuration(&self) -> PluginConfiguration {
        PluginConfiguration::default()
    }
}

#[derive(Default)]
pub struct QueuedNotifier {
    queue: RefCell<Vec<(EntityKey, Rc<dyn replies::Observed>)>>,
}

impl QueuedNotifier {
    pub fn forward(&self, receiver: &impl Notifier) -> Result<()> {
        let mut queue = self.queue.borrow_mut();

        for (audience, observed) in queue.iter() {
            receiver.notify(audience, observed)?;
        }

        queue.clear();

        Ok(())
    }
}

impl Notifier for QueuedNotifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn replies::Observed>) -> Result<()> {
        self.queue
            .borrow_mut()
            .push((audience.clone(), observed.clone()));

        Ok(())
    }
}

struct StandardOutNotifier {
    key: EntityKey,
}

impl StandardOutNotifier {
    fn new(key: &EntityKey) -> Self {
        Self { key: key.clone() }
    }
}

impl Notifier for StandardOutNotifier {
    fn notify(&self, audience: &EntityKey, observed: &Rc<dyn replies::Observed>) -> Result<()> {
        if *audience == self.key {
            let serialized = observed.to_json()?;
            println!("{:?}", serialized);
        }

        Ok(())
    }
}

async fn find_user_key(domain: &Domain, name: &str) -> Result<Option<EntityKey>> {
    tokio::task::spawn_blocking({
        let domain = domain.clone();
        let name = name.to_owned();

        move || {
            let session = domain.open_session().with_context(|| "Opening session")?;

            let maybe_key = session.find_name_key(&name)?;

            session.close(&DevNullNotifier::default())?;

            Ok(maybe_key)
        }
    })
    .await?
}

pub static TEXT_EXTENSION: &str = "txt";
pub static JSON_EXTENSION: &str = "json";

pub fn try_interactive(
    session: Rc<Session>,
    living: &EntityKey,
    reply: Box<dyn Reply>,
) -> Result<Box<dyn Reply>> {
    let value = reply.to_json()?;
    match &value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                // TODO This is annoying.
                if key == "editor" {
                    let reply: EditorReply = serde_json::from_value(value.clone())?;
                    let action: Box<dyn kernel::Action> = match reply.editing {
                        replies::WorkingCopy::Description(original) => {
                            let edited = default_external_editor(&original, TEXT_EXTENSION)?;

                            Box::new(SaveWorkingCopyAction {
                                key: EntityKey::new(&reply.key),
                                copy: replies::WorkingCopy::Description(edited),
                            })
                        }
                        replies::WorkingCopy::Json(original) => {
                            let serialized = serde_json::to_string_pretty(&original)?;
                            let edited = default_external_editor(&serialized, JSON_EXTENSION)?;

                            Box::new(SaveWorkingCopyAction {
                                key: EntityKey::new(&reply.key),
                                copy: replies::WorkingCopy::Json(serde_json::from_str(&edited)?),
                            })
                        }
                        replies::WorkingCopy::Script(original) => {
                            let edited = default_external_editor(&original, RUNE_EXTENSION)?;

                            Box::new(SaveScriptAction {
                                key: EntityKey::new(&reply.key),
                                copy: replies::WorkingCopy::Script(edited),
                            })
                        }
                    };

                    match session.entry(&kernel::LookupBy::Key(living))? {
                        Some(living) => return session.chain(Perform::Living { living, action }),
                        None => break,
                    }
                }
            }

            Ok(reply)
        }
        _ => Ok(reply),
    }
}

fn evaluate_commands(
    domain: engine::Domain,
    self_key: EntityKey,
    username: String,
    line: String,
) -> Result<()> {
    let session = domain.open_session().with_context(|| "Opening session")?;

    let reply: Box<dyn Reply> =
        if let Some(reply) = session.evaluate_and_perform(&username, &line)? {
            reply
        } else {
            Box::new(SimpleReply::What)
        };

    let renderer = Renderer::new(session.clone())?;
    let reply = try_interactive(session.clone(), &self_key, reply)?;
    let rendered = renderer.render_reply(&reply)?;

    let notifier = QueuedNotifier::default();

    session.close(&notifier)?;

    println!("{}", rendered);

    notifier.forward(&StandardOutNotifier::new(&self_key))?;

    Ok(())
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain(cmd.plugin_configuration()).await?;

    let self_key = find_user_key(&domain, &cmd.username)
        .await?
        .expect("No such username");

    let mut rl = DefaultEditor::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }

    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;

                let handle: JoinHandle<Result<()>> = tokio::task::spawn_blocking({
                    let username = cmd.username.clone();
                    let self_key = self_key.clone();
                    let domain = domain.clone();

                    || evaluate_commands(domain, self_key, username, line)
                });

                handle.await??;
            }
            Err(ReadlineError::Interrupted) => {
                println!("ctrl-c");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("ctrl-d");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    tokio::task::spawn_blocking(move || domain.stop()).await??;

    Ok(rl.save_history("history.txt")?)
}
