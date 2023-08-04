use yew::prelude::*;

use crate::shared::TextInput;

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub oncommand: Callback<String>,
}

#[function_component(CommandLine)]
pub fn command_line(props: &Props) -> Html {
    html! {
        <div class="command-line-editor">
            <TextInput value="" onsubmit={props.oncommand.clone()} />
        </div>
    }
}
