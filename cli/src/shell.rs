use anyhow::Result;
use clap::Args;
use plugins_core::building::actions::SaveWorkingCopyAction;
use replies::EditorReply;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::cell::RefCell;
use std::rc::Rc;

use crate::terminal::Renderer;
use crate::{make_domain, terminal::default_external_editor};
use engine::{self, DevNullNotifier, Domain, Notifier, Session};
use kernel::{ActiveSession, EntityKey, Reply, SimpleReply};

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
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

fn find_user_key(domain: &Domain, name: &str) -> Result<Option<EntityKey>> {
    let session = domain.open_session().expect("Error opening session");

    let maybe_key = session.find_name_key(name)?;

    session.close(&DevNullNotifier::default())?;

    Ok(maybe_key)
}

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
                    let save_action = match reply.editing {
                        replies::WorkingCopy::Description(original) => {
                            let edited = default_external_editor(&original, "txt")?;

                            SaveWorkingCopyAction {
                                key: EntityKey::new(&reply.key),
                                copy: replies::WorkingCopy::Description(edited),
                            }
                        }
                        replies::WorkingCopy::Json(original) => {
                            let serialized = serde_json::to_string_pretty(&original)?;
                            let edited = default_external_editor(&serialized, "txt")?;

                            SaveWorkingCopyAction {
                                key: EntityKey::new(&reply.key),
                                copy: replies::WorkingCopy::Json(serde_json::from_str(&edited)?),
                            }
                        }
                    };

                    match session.entry(&kernel::LookupBy::Key(&living))? {
                        Some(living) => return session.chain(&living, Box::new(save_action)),
                        None => break,
                    }
                }
            }

            Ok(reply)
        }
        _ => Ok(reply),
    }
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let domain = make_domain()?;

    let self_key = find_user_key(&domain, &cmd.username)?.expect("No such username");

    let mut rl = Editor::<()>::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
                let session = domain.open_session()?;

                let reply: Box<dyn Reply> = if let Some(reply) =
                    session.evaluate_and_perform(&cmd.username, line.as_str())?
                {
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
                println!();

                notifier.forward(&StandardOutNotifier::new(&self_key))?;
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
    Ok(rl.save_history("history.txt")?)
}
