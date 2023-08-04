use gloo_timers::callback::Timeout;
use web_sys::HtmlElement;
use yew::prelude::*;

use crate::shared::editor::Editor;
use crate::shared::history_items::HistoryItems;
use crate::shared::CommandLine;
use crate::shared::Evaluator;
use crate::shared::LogoutButton;
use crate::types::AllKnownItems;
use crate::types::{HistoryEntry, SessionHistory};

pub enum Msg {
    History(SessionHistory),
    Evaluator(Evaluator),
    Refresh,
}

pub struct Home {
    refs: Vec<NodeRef>,
    history: Option<SessionHistory>,
    evaluator: Evaluator,
    _history_listener: ContextHandle<SessionHistory>,
    _evaluator_listener: ContextHandle<Evaluator>,
}

impl Component for Home {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let (history, history_listener) = ctx
            .link()
            .context::<SessionHistory>(ctx.link().callback(|history| Msg::History(history)))
            .expect("No history context");

        let (evaluator, evaluator_listener) = ctx
            .link()
            .context::<Evaluator>(ctx.link().callback(|evaluator| Msg::Evaluator(evaluator)))
            .expect("No evalutor context");

        Self {
            history: Some(history),
            refs: vec![NodeRef::default()],
            evaluator,
            _history_listener: history_listener,
            _evaluator_listener: evaluator_listener,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::Evaluator(evaluator) => {
                log::trace!("update-evaluator");

                self.evaluator = evaluator;

                true
            }
            Self::Message::History(history) => {
                self.history = Some(history);

                log::trace!("update-history");

                let refresher = ctx.link().callback(|_| Self::Message::Refresh);

                let timeout = Timeout::new(25, move || {
                    refresher.emit(());
                });

                timeout.forget();

                true
            }
            Self::Message::Refresh => {
                let upper = &self.refs[0];
                let upper_div = &upper.cast::<HtmlElement>().unwrap();

                log::debug!(
                    "update-history:refresh ({}, {})",
                    upper_div.scroll_top(),
                    upper_div.scroll_height()
                );

                upper_div.set_scroll_top(upper_div.scroll_height());

                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        let evaluator = self.evaluator.clone();

        if let Some(history) = self.history.clone() {
            let latest = history.latest();

            html! {
                <div id="hack">
                    <div id="upper" ref={&self.refs[0]}>
                        <div id="main"><HistoryItems history={history} /></div>
                    </div>
                    <div id="lower">
                        <div class="interactables">
                            <BottomEditor {latest} />
                            <div class="bottom-bar">
                                <CommandLine oncommand={move |line: String| evaluator.evaluate(line.clone())} />
                                <LogoutButton />
                            </div>
                        </div>
                    </div>
                </div>
            }
        } else {
            html! { <div> {{ "Busy" }} </div> }
        }
    }
}

trait Editable {
    fn editor_text(&self) -> Result<String, serde_json::Error>;
    fn make_reply(&self, value: String) -> Result<String, serde_json::Error>;
}

impl Editable for replies::EditorReply {
    fn editor_text(&self) -> Result<String, serde_json::Error> {
        match self.editing() {
            replies::WorkingCopy::Description(value) => Ok(value.clone()),
            replies::WorkingCopy::Json(value) => serde_json::to_string_pretty(value),
            replies::WorkingCopy::Script(value) => Ok(value.clone()),
        }
    }

    fn make_reply(&self, value: String) -> Result<String, serde_json::Error> {
        let _copy = match self.editing() {
            replies::WorkingCopy::Description(_) => replies::WorkingCopy::Description(value),
            replies::WorkingCopy::Json(_) => {
                replies::WorkingCopy::Json(serde_json::from_str(&value)?)
            }
            replies::WorkingCopy::Script(_) => replies::WorkingCopy::Script(value),
        };

        /*
        let action = SaveWorkingCopyAction {
            key: EntityKey::new(self.key()),
            copy,
        };
        */

        Ok("{}".to_owned())
    }
}

#[derive(Properties, PartialEq)]
pub struct BottomEditorProps {
    latest: Option<HistoryEntry>,
}

use yew::html::RenderError;
use yew::suspense::*;

#[function_component(BottomEditor)]
pub fn bottom_editor(props: &BottomEditorProps) -> HtmlResult {
    let Some(latest) = props.latest.clone() else {
        return Ok(html! { <div></div> })
    };

    let known: Option<AllKnownItems> = latest.into();

    let Some(AllKnownItems::EditorReply(editor)) = &known else {
        return Ok(html! { <div></div> })
    };

    let code = editor
        .editor_text()
        .map_err(|_| RenderError::Suspended(Suspension::new().0))?; // .map_err(RenderError)?;

    let on_save = {
        let editor = editor.clone();
        Callback::from(move |code| {
            log::info!("on-save {:?}", code);
            match editor.make_reply(code) {
                Ok(_reply) => todo!(),
                Err(_) => todo!(),
            }
        })
    };

    Ok(html! {
        <Editor code={code} {on_save} />
    })
}
