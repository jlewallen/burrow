use replies::*;
use serde::Deserialize;
use yew::prelude::*;

use crate::{
    hooks::use_user_context,
    types::{AllKnownItems, HistoryEntry, Myself},
};

const NO_NAME: &str = "No Name";

fn simple_entities_list(entities: &Vec<ObservedEntity>) -> Html {
    let names = entities
        .iter()
        .map(|e| e.name.clone().or(Some(NO_NAME.into())).unwrap())
        .collect::<Vec<_>>()
        .join(", ");

    html! {
        <span class="entities">{ names }</span>
    }
}

fn area_observation(reply: &AreaObservation) -> Html {
    let name: &str = if let Some(name) = &reply.area.name {
        &name
    } else {
        NO_NAME
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

trait Render {
    fn render(&self, myself: &Myself) -> Html;
}

impl Render for AllKnownItems {
    fn render(&self, myself: &Myself) -> Html {
        match self {
            Self::AreaObservation(reply) => area_observation(&reply),
            Self::InsideObservation(reply) => inside_observation(&reply),
            Self::SimpleReply(reply) => simple_reply(&reply),
            Self::SimpleObservation(reply) => simple_observation(&reply, myself),
            Self::EntityObservation(_) => todo!(),
            Self::EditorReply(_) => html! {
                <div class="entry hidden"> { "Opening editor..." } </div>
            },
            Self::JsonReply(_) => todo!(),
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub entry: HistoryEntry,
}

#[function_component(HistoryEntryItem)]
pub fn history_entry_item(props: &Props) -> Html {
    let user = use_user_context();
    let key = user.key().expect("expected authenticated user with key");
    let myself = Myself {
        key: Some(key.clone()),
    };

    let value = &props.entry.value;
    if let Ok(item) = serde_json::from_value::<AllKnownItems>(value.clone()) {
        item.render(&myself)
    } else {
        html! {
            <div class="entry unknown">
                { value.to_string() }
            </div>
        }
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
