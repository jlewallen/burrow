use replies::{EditorReply, JsonValue};
use wasm_bindgen::JsCast;
use web_sys::HtmlElement;
use yew::html::RenderError;
use yew::prelude::*;
use yew::suspense::*;
use yew_hooks::use_timeout;

use crate::shared::editor::Editor;
use crate::shared::history_items::HistoryItems;
use crate::shared::CommandLine;
use crate::shared::Evaluator;
use crate::shared::LogoutButton;
use crate::types::AllKnownItems;
use crate::types::SessionHistory;

#[allow(dead_code)]
const GIT_HASH: &str = env!("GIT_HASH");

#[function_component(Home)]
pub fn home() -> Html {
    let evaluator = use_context::<Evaluator>();
    let Some(evaluator) = evaluator else  {
        log::info!("home: no evaluator");
        return html! { <div></div> }
    };

    let history = use_context::<SessionHistory>();
    let Some(history) = history else  {
        log::info!("home: no history");
        return html! { <div></div> }
    };

    let upper_ref = use_node_ref();
    let scroll_delay = {
        let upper_ref = upper_ref.clone();

        use_timeout(
            move || {
                let upper_div = &upper_ref.cast::<HtmlElement>().unwrap();

                log::trace!(
                    "update-history:refresh ({}, {})",
                    upper_div.scroll_top(),
                    upper_div.scroll_height()
                );

                upper_div.set_scroll_top(upper_div.scroll_height());
            },
            25,
        )
    };

    use_effect_with_deps(
        move |(_history,)| {
            scroll_delay.reset();
        },
        (history.clone(),),
    );

    let body_click = {
        Callback::from(|_| {
            focus_command_line();
        })
    };

    html! {
        <div id="hack" onclick={body_click}>
            <div id="upper" ref={upper_ref}>
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
}

trait Editable {
    fn editor_text(&self) -> Result<String, serde_json::Error>;
    fn make_save_action(&self, value: String) -> Result<JsonValue, serde_json::Error>;
    fn language(&self) -> &str;
}

impl Editable for replies::EditorReply {
    fn editor_text(&self) -> Result<String, serde_json::Error> {
        match self.editing() {
            replies::WorkingCopy::Markdown(value) => Ok(value.clone()),
            replies::WorkingCopy::Json(value) => serde_json::to_string_pretty(value),
            replies::WorkingCopy::Script(value) => Ok(value.clone()),
        }
    }

    fn make_save_action(&self, value: String) -> Result<JsonValue, serde_json::Error> {
        let value = match self.editing() {
            replies::WorkingCopy::Markdown(_) => JsonValue::String(value),
            replies::WorkingCopy::Json(_) => serde_json::from_str(&value)?,
            replies::WorkingCopy::Script(_) => JsonValue::String(value),
        };

        return Ok(self.save().clone().instantiate(&value));
    }

    fn language(&self) -> &str {
        match self.editing() {
            replies::WorkingCopy::Markdown(_) => "markdown",
            replies::WorkingCopy::Json(_) => "json",
            replies::WorkingCopy::Script(_) => "rust",
        }
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
            <Editor code={editing.editor_text().map_err(|_| RenderError::Suspended(Suspension::new().0))?}
                language={editing.language().to_owned()} {on_save} {on_quit} />
        } else {
            <div></div>
        }
    })
}
