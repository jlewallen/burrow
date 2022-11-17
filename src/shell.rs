use crate::domain;
use crate::kernel::{Reply, SimpleReply};
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

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let renderer = Renderer::new()?;
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

                let reply: Box<dyn Reply> = if let Some(reply) =
                    session.evaluate_and_perform(&cmd.username, line.as_str())?
                {
                    reply
                } else {
                    Box::new(SimpleReply::What)
                };

                let rendered = renderer.render(reply)?;

                session.close()?;

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
