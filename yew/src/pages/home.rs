use gloo_timers::callback::Timeout;
use replies::EditorReply;
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use yew::html::RenderError;
use yew::prelude::*;
use yew::suspense::*;

use crate::shared::editor::Editor;
use crate::shared::history_items::HistoryItems;
use crate::shared::CommandLine;
use crate::shared::Evaluator;
use crate::shared::LogoutButton;
use crate::types::AllKnownItems;
use crate::types::SaveWorkingCopyAction;
use crate::types::SessionHistory;

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
            html! {
                <div id="hack">
                    <div id="upper" ref={&self.refs[0]}>
                        <div id="main"><HistoryItems history={history} /></div>
                    </div>
                    <div id="lower">
                        <div class="interactables">
                            <BottomEditor />
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
    fn make_save_action(&self, value: String) -> Result<serde_json::Value, serde_json::Error>;
}

impl Editable for replies::EditorReply {
    fn editor_text(&self) -> Result<String, serde_json::Error> {
        match self.editing() {
            replies::WorkingCopy::Description(value) => Ok(value.clone()),
            replies::WorkingCopy::Json(value) => serde_json::to_string_pretty(value),
            replies::WorkingCopy::Script(value) => Ok(value.clone()),
        }
    }

    fn make_save_action(&self, value: String) -> Result<serde_json::Value, serde_json::Error> {
        let copy = match self.editing() {
            replies::WorkingCopy::Description(_) => replies::WorkingCopy::Description(value),
            replies::WorkingCopy::Json(_) => {
                replies::WorkingCopy::Json(serde_json::from_str(&value)?)
            }
            replies::WorkingCopy::Script(_) => replies::WorkingCopy::Script(value),
        };

        let action = SaveWorkingCopyAction {
            key: self.key().to_owned(),
            copy,
        };

        serde_json::to_value(action)
    }
}

pub fn focus_command_line() {
    let document = web_sys::window().unwrap().document().unwrap();
    let commands = document.get_element_by_id("command-line");
    let commands = commands.unwrap();
    let commands = commands
        .dyn_into::<web_sys::HtmlInputElement>()
        .expect("html cast error");
    commands.focus().expect("focus error");
}

#[function_component(BottomEditor)]
pub fn bottom_editor() -> HtmlResult {
    let evaluator = use_context::<Evaluator>();
    let Some(evaluator) = evaluator else  {
        log::info!("editor: no evaluator");
        return Ok(html! { <div></div> })
    };

    let editing = use_state(|| None::<EditorReply>);

    {
        let editing = editing.clone();
        let history = use_context::<SessionHistory>();
        use_effect_with_deps(
            move |(history,)| {
                if let Some(history) = history {
                    let latest = history.latest();
                    if let Some(latest) = latest.clone() {
                        let known: Option<AllKnownItems> = latest.into();
                        if let Some(AllKnownItems::EditorReply(reply)) = &known {
                            editing.set(Some(reply.clone()));
                        };
                    };
                };
            },
            (history,),
        );
    }

    let on_quit = {
        let editing = editing.clone();
        Callback::from(move |_| {
            log::info!("on-quit");
            editing.set(None);
            focus_command_line();
        })
    };

    let on_save = {
        let editing = editing.clone();
        Callback::from(move |code| {
            log::trace!("on-save {:?}", code);
            if let Some(original) = editing.as_ref() {
                match original.make_save_action(code) {
                    Ok(action) => {
                        evaluator.perform(action);
                        editing.set(None);
                        focus_command_line();
                    }
                    Err(e) => log::error!("error making save action: {:?}", e),
                }
            }
        })
    };

    Ok(html! {
        if let Some(editing) = editing.as_ref() {
            <Editor code={editing.editor_text().map_err(|_| RenderError::Suspended(Suspension::new().0))?} {on_save} {on_quit} />
        } else {
            <div></div>
        }
    })
}
