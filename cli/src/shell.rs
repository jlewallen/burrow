use anyhow::Result;
use chrono::Utc;
use clap::Args;
use plugins_rune::RUNE_EXTENSION;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::cell::RefCell;
use std::rc::Rc;
use tokio::task::JoinHandle;
use tokio::time::sleep;
use tracing::*;

use crate::rpc::try_parse_action;
use crate::terminal::Renderer;
use crate::{terminal::default_external_editor, DomainBuilder};

use engine::{self, DevNullNotifier, Domain, HasUsernames, Notifier, SessionOpener};
use kernel::common::SimpleReply;
use kernel::prelude::{
    get_my_session, Effect, EffectReply, EntityKey, EntryResolver, JsonValue, Middleware, Perform,
    PerformAction,
};
use replies::{EditorReply, TaggedJson};

#[derive(Debug, Args)]
pub struct Command {
    #[arg(short, long, value_name = "FILE")]
    path: Option<String>,
    #[arg(short, long, default_value = "jlewallen")]
    username: String,
}

impl Command {
    fn builder(&self) -> DomainBuilder {
        DomainBuilder::new(self.path.clone())
    }
}

#[derive(Default)]
pub struct QueuedNotifier {
    queue: RefCell<Vec<(EntityKey, TaggedJson)>>,
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
    fn notify(&self, audience: &EntityKey, observed: &TaggedJson) -> Result<()> {
        self.queue
            .borrow_mut()
            .push((audience.clone(), observed.clone()));

        Ok(())
    }
}

struct StandardOutNotifier {
    key: EntityKey,
    renderer: crate::text::Renderer,
}

impl StandardOutNotifier {
    fn new(key: &EntityKey, renderer: crate::text::Renderer) -> Self {
        Self {
            key: key.clone(),
            renderer,
        }
    }
}

impl Notifier for StandardOutNotifier {
    fn notify(&self, audience: &EntityKey, observed: &TaggedJson) -> Result<()> {
        if *audience == self.key {
            let value = observed.clone().into_tagged();
            let rendererd = self.renderer.render_value(&value)?;
            println!("{}", rendererd);
        }

        Ok(())
    }
}

async fn find_user_key(domain: &Domain, name: &str) -> Result<Option<EntityKey>> {
    tokio::task::spawn_blocking({
        let domain = domain.clone();
        let name = name.to_owned();

        move || {
            let session = domain.open_session()?;

            let world = session.world()?.expect("No world");
            let maybe_key = world.find_name_key(&name)?;

            session.close(&DevNullNotifier::default())?;

            Ok(maybe_key)
        }
    })
    .await?
}

pub static MD_EXTENSION: &str = "md";
#[allow(dead_code)]
pub static TEXT_EXTENSION: &str = "txt";
pub static JSON_EXTENSION: &str = "json";

pub struct InteractiveEditor {
    living: EntityKey,
}

impl Middleware for InteractiveEditor {
    fn handle(
        &self,
        value: Perform,
        next: kernel::prelude::MiddlewareNext,
    ) -> Result<Effect, anyhow::Error> {
        match next.handle(value)? {
            Effect::Reply(reply) => {
                match reply.clone() {
                    kernel::prelude::EffectReply::TaggedJson(tagged) => {
                        let value = tagged.into_tagged();
                        match &value {
                            JsonValue::Object(object) => {
                                for (key, value) in object {
                                    // TODO This is annoying.
                                    if key == "editorReply" {
                                        let reply: EditorReply =
                                            serde_json::from_value(value.clone())?;
                                        let value: JsonValue = match reply.editing() {
                                            replies::WorkingCopy::Markdown(original) => {
                                                JsonValue::String(default_external_editor(
                                                    &original,
                                                    MD_EXTENSION,
                                                )?)
                                            }
                                            replies::WorkingCopy::Json(original) => {
                                                let serialized =
                                                    serde_json::to_string_pretty(&original)?;
                                                serde_json::from_str(&default_external_editor(
                                                    &serialized,
                                                    JSON_EXTENSION,
                                                )?)?
                                            }
                                            replies::WorkingCopy::Script(original) => {
                                                JsonValue::String(default_external_editor(
                                                    &original,
                                                    RUNE_EXTENSION,
                                                )?)
                                            }
                                        };

                                        let value = reply.save().clone().instantiate(&value);

                                        let action: Rc<_> = try_parse_action(value)
                                            .expect("try parse action failed")
                                            .into();

                                        let session = get_my_session()?;
                                        match session
                                            .entry(&kernel::prelude::LookupBy::Key(&self.living))?
                                        {
                                            Some(living) => {
                                                return session.perform(Perform::Living {
                                                    living,
                                                    action: PerformAction::Instance(action),
                                                });
                                            }
                                            None => break,
                                        }
                                    }
                                }

                                Ok(Effect::Reply(reply))
                            }
                            _ => Ok(Effect::Reply(reply)),
                        }
                    }
                }
            }
            effect => Ok(effect),
        }
    }
}

impl InteractiveEditor {}

fn evaluate_commands(
    domain: engine::Domain,
    self_key: EntityKey,
    username: String,
    line: String,
) -> Result<()> {
    let interactive = Rc::new(InteractiveEditor {
        living: self_key.clone(),
    });
    let session = domain.open_session_with_middleware(vec![interactive])?;

    let effect: Effect = if let Some(effect) = session.evaluate_and_perform(&username, &line)? {
        effect
    } else {
        SimpleReply::What.try_into()?
    };

    let notifier = QueuedNotifier::default();
    session.close(&notifier)?;

    let text = crate::text::Renderer::new()?;
    let renderer = Renderer::new(session.clone(), text.clone())?;

    let rendered = match effect {
        Effect::Reply(reply) => match reply {
            EffectReply::TaggedJson(tagged) => Some(renderer.render_value(&tagged.into_tagged())?),
        },
        Effect::Ok => None,
        _ => todo!(),
    };

    if let Some(rendered) = rendered {
        println!("{}", rendered);
    }
    notifier.forward(&StandardOutNotifier::new(&self_key, text))?;

    Ok(())
}

#[tokio::main]
pub async fn execute_command(cmd: &Command) -> Result<()> {
    let builder = cmd.builder();
    let domain = builder.build().await?;

    let self_key = find_user_key(&domain, &cmd.username)
        .await?
        .expect("No such username");

    let mut rl = DefaultEditor::new()?;
    if rl.load_history("history.txt").is_err() {
        println!("No previous history.");
    }

    let text = crate::text::Renderer::new()?;

    tokio::task::spawn({
        let domain = domain.clone();
        let self_key = self_key.clone();
        let standard_out = StandardOutNotifier::new(&self_key, text);

        async move {
            loop {
                sleep(std::time::Duration::from_secs(1)).await;

                let notifier = QueuedNotifier::default();
                let now = Utc::now();
                if let Err(e) = domain.tick(now, &notifier) {
                    warn!("tick failed: {:?}", e);
                }

                if let Err(e) = notifier.forward(&standard_out) {
                    warn!("tick failed forwarding notifications: {:?}", e);
                }
            }
        }
    });

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
