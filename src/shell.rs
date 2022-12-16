use crate::domain::{self, DevNullNotifier, Domain, Notifier};
use crate::kernel::{EntityKey, Reply, SimpleReply};
use crate::storage;
use crate::text::Renderer;
use anyhow::Result;
use clap::Args;
use rustyline::error::ReadlineError;
use rustyline::Editor;

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
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
    fn notify(&self, audience: EntityKey, observed: Box<dyn replies::Observed>) -> Result<()> {
        if audience == self.key {
            let serialized = observed.to_json()?;
            println!("{:?}", serialized);
        }

        Ok(())
    }
}

fn find_user_key(domain: &Domain, name: &str) -> Result<Option<EntityKey>> {
    let session = domain.open_session().expect("Error opening session");

    let maybe_key = session.find_name_key(name)?;

    session.close(&DevNullNotifier::new())?;

    Ok(maybe_key)
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let renderer = Renderer::new()?;
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = domain::Domain::new(storage_factory, false);

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

                session.close(&StandardOutNotifier::new(&self_key))?;

                println!("{}", rendered);
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
