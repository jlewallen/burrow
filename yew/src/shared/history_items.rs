use yew::prelude::*;

use super::HistoryEntryItem;
use super::SessionHistory;

#[derive(Properties, Clone, PartialEq, Eq)]
pub struct Props {
    pub history: SessionHistory,
}

#[function_component(HistoryItems)]
pub fn history_items(props: &Props) -> Html {
    html! {
        <div class="history">
            <div class="entries">
                { for props.history.entries.iter().map(|entry| html!{ <HistoryEntryItem entry={entry.clone()} /> }) }
            </div>
        </div>
    }
}
