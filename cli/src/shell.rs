use anyhow::Result;
use clap::Args;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use std::cell::RefCell;
use std::rc::Rc;

use crate::make_domain;
use crate::text::Renderer;
use engine::{self, DevNullNotifier, Domain, Notifier};
use kernel::{EntityKey, Reply, SimpleReply};

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

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let renderer = Renderer::new()?;
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

                let rendered = renderer.render(reply)?;

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
