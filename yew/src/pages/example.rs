// use monaco::api::TextModel;
// use monaco::{api::CodeEditorOptions, sys::editor::BuiltinTheme, yew::CodeEditor};
use yew::prelude::*;

// use yew_hooks::prelude::*;
// use yew_router::prelude::*;

// use crate::hooks::*;
// use crate::routes::*;
// use crate::services::*;
// use crate::shared::*;
// use crate::types::*;

#[function_component(Example)]
pub fn example_page() -> Html {
    html! {
        <div>{ "Example" }</div>
    }
    /*
    let text_model =
        use_state_eq(|| TextModel::create(&random_string(), Some("rust"), None).unwrap());

    let on_run_clicked = {
        let text_model = text_model.clone();
        use_callback(
            move |_, text_model| {
                let s: String = random_string();
                // Here we have full access to the text model. We can do whatever we want with
                // it. For this example, we'll just set the value to a random
                // string.
                text_model.set_value(&s);
            },
            text_model,
        )
    };

    html! {
        <div id="code-wrapper">
            <div id="code-editor">
                <CustomEditor text_model={(*text_model).clone()} />
            </div>
            <div id="event-log-wrapper">
                <div id="event-log">
                    <button onclick={on_run_clicked}>{ "Random code" }</button>
                </div>
            </div>
        </div>
    }
    */
}
