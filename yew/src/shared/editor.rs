use monaco::api::TextModel;
use monaco::sys::editor::IStandaloneCodeEditor;
use monaco::yew::CodeEditorLink;
use monaco::{api::CodeEditorOptions, sys::editor::BuiltinTheme, yew::CodeEditor};
use wasm_bindgen::prelude::Closure;
use wasm_bindgen::JsCast;
use yew::prelude::*;

fn get_options() -> CodeEditorOptions {
    CodeEditorOptions::default()
        .with_language("rust".to_owned())
        .with_value("".to_owned())
        .with_builtin_theme(BuiltinTheme::VsDark)
        .with_automatic_layout(true)
}

#[derive(PartialEq, Properties)]
pub struct CustomEditorProps {
    text_model: TextModel,
    on_editor_created: Callback<CodeEditorLink>,
}

/// This is really just a helper component, so we can pass in props easier.
/// It makes it much easier to use, as we can pass in what we need, and it
/// will only re-render if the props change.
#[function_component(CustomEditor)]
pub fn custom_editor(props: &CustomEditorProps) -> Html {
    let CustomEditorProps {
        on_editor_created,
        text_model,
    } = props;

    html! {
        <CodeEditor classes={"full-height"} options={ get_options().to_sys_options() } {on_editor_created} model={text_model.clone()} />
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub code: String,
    pub on_save: Callback<String>,
}

#[function_component(Editor)]
pub fn editor(props: &Props) -> Html {
    let text_model = use_state_eq(|| TextModel::create(&props.code, Some("rust"), None).unwrap());

    // This is the current code output. As it's static from the example, we set it to the content.
    let on_save = props.on_save.clone();

    // Here we setup the Callback for when the editor is created.
    let on_editor_created = {
        // We need to clone the text_model/code so we can use them.
        let text_model = text_model.clone();

        // This is a javascript closure, used to pass to Monaco, using wasm-bindgen.
        let js_closure = {
            let text_model = text_model.clone();

            // We update the code state when the Monaco model changes.
            // See https://yew.rs/docs/0.20.0/concepts/function-components/pre-defined-hooks
            Closure::<dyn Fn()>::new(move || {
                log::info!("changed");
                // code.set(text_model.get_value());
                on_save.emit(text_model.get_value());
            })
        };

        // Here we define our callback, we use use_callback as we want to re-render when dependencies change.
        // See https://yew.rs/docs/concepts/function-components/state#general-view-of-how-to-store-state
        use_callback(
            move |editor_link: CodeEditorLink, _text_model| {
                editor_link.with_editor(|editor| {
                    // Registers Ctrl/Cmd + Enter hotkey
                    let keycode = monaco::sys::KeyCode::Enter.to_value()
                        | (monaco::sys::KeyMod::ctrl_cmd() as u32);
                    let raw_editor: &IStandaloneCodeEditor = editor.as_ref();

                    raw_editor.add_command(
                        keycode.into(),
                        js_closure.as_ref().unchecked_ref(),
                        None,
                    );
                });
            },
            text_model,
        )
    };

    html! {
        <div class="bottom-editor">
            <CustomEditor text_model={(*text_model).clone()} {on_editor_created} />
        </div>
    }
}
