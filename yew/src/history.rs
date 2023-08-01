// use gloo_console as console;
use crate::open_web_socket::Myself;
use replies::*;
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use yew::prelude::*;
use yewdux::prelude::*;

#[derive(Debug, Serialize, Clone, Eq, PartialEq)]
pub struct HistoryEntry {
    pub value: serde_json::Value,
}

impl HistoryEntry {
    pub fn new(value: serde_json::Value) -> Self {
        Self { value }
    }
}

#[derive(Default, Store, PartialEq)]
pub struct SessionHistory {
    entries: Vec<HistoryEntry>,
}

impl SessionHistory {
    pub fn append(&self, value: serde_json::Value) -> Self {
        let entries = if !value.is_null() {
            self.entries
                .clone()
                .into_iter()
                .chain([HistoryEntry::new(value)])
                .collect()
        } else {
            self.entries.clone()
        };
        Self { entries }
    }
}

fn simple_entities_list(entities: &Vec<ObservedEntity>) -> Html {
    let names = entities
        .iter()
        .map(
            // TODO Super awkward clone, I'm tired though.
            |e| /* html! { <span> { */ e.name.clone().or(Some("?".into())).unwrap(), /* } </span> } */
        )
        .collect::<Vec<_>>()
        // .intersperse(", "); // TODO This may be a thing some day and
        // would be nice if this had Html returned above, maybe.
        .join(", ");

    html! {
        <span class="entities">{ names }</span>
    }
}

fn area_observation(reply: &AreaObservation) -> Html {
    let name: &str = if let Some(name) = &reply.area.name {
        &name
    } else {
        "No Name"
    };

    let desc: Html = if let Some(desc) = &reply.area.desc {
        html! {
            <p class="desc">{ desc }</p>
        }
    } else {
        html! { <span></span> }
    };

    let living: Html = if reply.living.len() > 0 {
        html! {
            <div class="living">
                { "Also here is "} { simple_entities_list(&reply.living) } { "." }
            </div>
        }
    } else {
        html! {<span></span>}
    };

    let items: Html = if reply.items.len() > 0 {
        html! {
            <div class="ground">
                { "You can see "} { simple_entities_list(&reply.items) } { "." }
            </div>
        }
    } else {
        html! {<span></span>}
    };

    let carrying: Html = if reply.carrying.len() > 0 {
        html! {
            <div class="hands">
                { "You are holding "} { simple_entities_list(&reply.carrying) } { "." }
            </div>
        }
    } else {
        html! {<span></span>}
    };

    let routes: Html = if reply.routes.len() > 0 {
        html! {
            <div class="routes">
                { "You can see "} { simple_entities_list(&reply.routes) } { "." }
            </div>
        }
    } else {
        html! {<span></span>}
    };

    html! {
        <div class="entry observation area">
            <h3>{ name }</h3>
            { desc }
            { routes }
            { living }
            { items }
            { carrying }
        </div>
    }
}

fn inside_observation(reply: &InsideObservation) -> Html {
    html! {
        <div class="living observation inside">
            { "Inside is "}{ simple_entities_list(&reply.items) }{ "." }
        </div>
    }
}

#[derive(Debug, Deserialize)]
struct ItemHeld {
    living: ObservedEntity,
    item: ObservedEntity,
}

#[derive(Debug, Deserialize)]
struct ItemDropped {
    living: ObservedEntity,
    item: ObservedEntity,
}

#[derive(Debug, Deserialize)]
struct LivingLeft {
    living: ObservedEntity,
}

#[derive(Debug, Deserialize)]
struct LivingArrived {
    living: ObservedEntity,
}

#[derive(Debug, Deserialize)]
struct KnownSimpleObservations {
    left: Option<LivingLeft>,
    arrived: Option<LivingArrived>,
    held: Option<ItemHeld>,
    dropped: Option<ItemDropped>,
}

fn simple_observation(reply: &SimpleObservation, myself: &Myself) -> Html {
    // I'm going to love cleaning this up later. Considering a quick function
    // for the "You" vs name work. We also need to introduce inflections of
    // various kinds. I think this will become critical when we've got
    // quantities working.
    if let Ok(reply) = serde_json::from_value::<KnownSimpleObservations>(reply.clone().into()) {
        if let Some(reply) = reply.left {
            if Some(reply.living.key) != myself.key {
                html! {
                    <div class="entry observation simple living-left">{ reply.living.name }{ " left." }</div>
                }
            } else {
                html! { <div></div> }
            }
        } else if let Some(reply) = reply.arrived {
            if Some(reply.living.key) != myself.key {
                html! {
                    <div class="entry observation simple living-arrived">{ reply.living.name } { " arrived." }</div>
                }
            } else {
                html! { <div></div> }
            }
        } else if let Some(reply) = reply.held {
            if Some(reply.living.key) == myself.key {
                html! {
                    <div class="entry observation simple item-held">{ "You picked up " }{ reply.item.name }</div>
                }
            } else {
                html! {
                    <div class="entry observation simple item-held">{ reply.living.name }{ " held " }{ reply.item.name }</div>
                }
            }
        } else if let Some(reply) = reply.dropped {
            if Some(reply.living.key) == myself.key {
                html! {
                    <div class="entry observation simple item-dropped">{ "You dropped " }{ reply.item.name }</div>
                }
            } else {
                html! {
                    <div class="entry observation simple item-dropped">{ reply.living.name }{ " dropped " }{ reply.item.name }</div>
                }
            }
        } else {
            html! {
                <div class="entry observation simple missing">{ "Missing: " }{ format!("{:?}", reply) }</div>
            }
        }
    } else {
        html! {
            <div class="entry observation simple unknown">{ "Unknown: " }{ format!("{:?}", reply) }</div>
        }
    }
}

fn simple_reply(reply: &SimpleReply) -> Html {
    html! {
        <div class="entry simple">{ format!("{:?}", reply) }</div>
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub entry: HistoryEntry,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BasicReply {
    Simple(SimpleReply),
    EntityObservation(EntityObservation),
    InsideObservation(InsideObservation),
    AreaObservation(AreaObservation),
    SimpleObservation(SimpleObservation),
    Editor(EditorReply),
    Json(JsonReply),
}

#[function_component]
pub fn HistoryEntryItem(props: &Props) -> Html {
    let myself = use_context::<Myself>().expect("No myself context");
    log::debug!("myself: {:?}", myself);

    let value = &props.entry.value;
    if let Ok(reply) = serde_json::from_value::<BasicReply>(value.clone()) {
        match reply {
            BasicReply::AreaObservation(reply) => area_observation(&reply),
            BasicReply::InsideObservation(reply) => inside_observation(&reply),
            BasicReply::SimpleObservation(reply) => simple_observation(&reply, &myself),
            BasicReply::Simple(reply) => simple_reply(&reply),
            BasicReply::EntityObservation(_) => todo!(),
            BasicReply::Editor(_) => todo!(),
            BasicReply::Json(_) => todo!(),
        }
    } else {
        html! {
            <div class="entry unknown">
                { props.entry.value.to_string() }
            </div>
        }
    }
}

pub enum Msg {
    UpdateHistory(std::rc::Rc<SessionHistory>),
}

pub struct History {
    history: Rc<SessionHistory>,
    #[allow(dead_code)]
    dispatch: Dispatch<SessionHistory>,
}

impl Component for History {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let callback = ctx.link().callback(Msg::UpdateHistory);
        let dispatch = Dispatch::<SessionHistory>::subscribe(move |h| callback.emit(h));

        Self {
            history: dispatch.get(),
            dispatch,
        }
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Msg::UpdateHistory(history) => {
                self.history = history;

                true
            }
        }
    }

    fn view(&self, _ctx: &Context<Self>) -> Html {
        html! {
            <div class="history">
                <div class="entries">
                    { for self.history.entries.iter().map(|entry| html!{ <HistoryEntryItem entry={entry.clone()} /> }) }
                </div>
            </div>
        }
    }
}
