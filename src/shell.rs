use crate::domain;
use crate::storage;
use anyhow::Result;
use clap::Args;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use tracing::*;

#[derive(Debug, Args)]
pub struct Command {}

#[tokio::main]
pub async fn execute_command(_cmd: &Command) -> Result<()> {
    let storage_factory = storage::sqlite::Factory::new("world.sqlite3")?;
    let domain = domain::Domain::new(storage_factory);

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

                let reply = session.evaluate_and_perform("jlewallen", line.as_str())?;

                info!("reply `{}`", reply.to_json()?);

                session.close()?
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
