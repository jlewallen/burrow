use replies::*;
use yew::prelude::*;

use crate::{
    hooks::use_user_context,
    types::{AllKnownItems, CarryingEvent, HistoryEntry, MovingEvent, Myself, TalkingEvent},
};

const NO_NAME: &str = "No Name";

fn simple_entities_list(entities: &Vec<ObservedEntity>) -> Html {
    let names = entities
        .iter()
        .map(|e| e.qualified.clone().or(Some(NO_NAME.into())).unwrap())
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
        let after_markdown = markdown::to_html(desc);
        let desc = Html::from_html_unchecked(AttrValue::from(after_markdown));
        html! {
            <div class="desc">{ desc }</div>
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

fn simple_reply(reply: &SimpleReply) -> Html {
    html! {
        <div class="entry simple">{ format!("{:?}", reply) }</div>
    }
}

trait Render {
    fn render(&self, myself: &Myself) -> Option<Html>;
}

impl Render for AllKnownItems {
    fn render(&self, myself: &Myself) -> Option<Html> {
        match self {
            Self::AreaObservation(reply) => Some(area_observation(&reply)),
            Self::InsideObservation(reply) => Some(inside_observation(&reply)),
            Self::SimpleReply(reply) => Some(simple_reply(&reply)),
            Self::CarryingEvent(event) => event.render(myself),
            Self::MovingEvent(event) => event.render(myself),
            Self::TalkingEvent(event) => event.render(myself),
            Self::EditorReply(_) => None,
            Self::EntityObservation(_) => todo!(),
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
        match item.render(&myself) {
            Some(html) => html,
            None => html! {},
        }
    } else {
        html! {
            <div class="entry unknown">
                { value.to_string() }
            </div>
        }
    }
}

fn subject(e: &ObservedEntity) -> Html {
    html! { <span>{ e.qualified.as_ref().unwrap() }</span> }
}

fn thing(e: &ObservedEntity) -> Html {
    html! { <span>{ e.qualified.as_ref().unwrap() }</span> }
}

impl Render for CarryingEvent {
    fn render(&self, myself: &Myself) -> Option<Html> {
        match self {
            CarryingEvent::ItemHeld {
                living,
                item,
                area: _,
            } => Some(
                html! { <div class="entry"> { subject(living) } { " picked up " } { thing(item) }</div> },
            ),
            CarryingEvent::ItemDropped {
                living,
                item,
                area: _,
            } => Some(
                html! { <div class="entry"> { subject(living) } { " dropped " } { thing(item) }</div> },
            ),
        }
    }
}

impl Render for MovingEvent {
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            MovingEvent::Left { living, area: _ } => {
                Some(html! { <div class="entry"> { subject(living) } { " left." } </div> })
            }
            MovingEvent::Arrived { living, area: _ } => {
                Some(html! { <div class="entry"> { subject(living) } { " arrived." } </div> })
            }
        }
    }
}

impl Render for TalkingEvent {
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            TalkingEvent::Conversation(s) => Some(
                html! { <div class="entry"> <span class="speaker">{ s.who.name.as_ref().unwrap() }</span>{ ": " } { &s.message } </div> },
            ),
            TalkingEvent::Whispering(_) => todo!(),
        }
    }
}
