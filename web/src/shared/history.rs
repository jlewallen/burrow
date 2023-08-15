use regex::Captures;
use replies::*;
use yew::prelude::*;

use crate::{
    hooks::use_user_context,
    types::{AllKnownItems, HistoryEntityPtr, Myself},
};

const NO_NAME: &str = "No Name";

const WIKI_WORD: &str = "([A-Z]+[a-z]+([A-Z]+[a-z]+)+)";

fn md_string(s: &str) -> Html {
    let r = regex::Regex::new(WIKI_WORD).expect("Wiki word regex error");
    let replacer = |caps: &Captures| -> String { caps[0].to_owned() };

    let after_markdown = markdown::to_html(&r.replace_all(s, &replacer));
    Html::from_html_unchecked(AttrValue::from(after_markdown))
}

fn join_html(v: Vec<Html>, separator: Html) -> Vec<Html> {
    // I'm so frustrated, why can't we use join here?
    let last_index = v.len() - 1;
    v.into_iter()
        .enumerate()
        .map(|(i, item)| {
            if i == last_index {
                vec![item]
            } else {
                vec![item, separator.clone()]
            }
        })
        .flatten()
        .collect()
}

fn simple_entities_list(entities: &Vec<ObservedEntity>) -> Html {
    let entities = entities
        .iter()
        .map(|e| (e.qualified.clone().or(Some(NO_NAME.into())).unwrap(), e.gid))
        .map(|(name, gid)| html!(<>{ name }{ NBSP }{ gid_span(gid) }</>))
        .collect::<Vec<_>>();
    let separator = html! { { ", " } };

    html! {
        <span class="entities">{ join_html(entities, separator) }</span>
    }
}

fn gid_span(gid: u64) -> Html {
    html! {
        <span class="gid">{ "(#" }{ gid }{ ")" }</span>
    }
}

const NBSP: &str = "\u{00a0}";

fn entity_name_desc(entity: &ObservedEntity) -> (Html, Html) {
    let gid = gid_span(entity.gid);
    let name: Html = if let Some(name) = &entity.name {
        html! { <h3> { name }{ NBSP }{ gid } </h3> }
    } else {
        html! { <h3> { NO_NAME } </h3> }
    };

    let desc: Html = if let Some(desc) = &entity.desc {
        let desc = md_string(desc);
        html! {
            <div class="desc">{ desc }</div>
        }
    } else {
        html! { <span></span> }
    };

    (name, desc)
}

fn entity_observation(observation: &EntityObservation) -> Html {
    let (name, desc) = entity_name_desc(&observation.entity);

    html! {
        <div class="entry observation entity">
            { name }
            { desc }
            if let Some(wearing) = &observation.wearing {
                <div class="wearing">
                    { "They are wearing "} { simple_entities_list(&wearing) } { "." }
                </div>
            }
        </div>
    }
}

fn area_observation(reply: &AreaObservation) -> Html {
    let (name, desc) = entity_name_desc(&reply.area);

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
            { name }
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

fn markdown_reply(reply: &MarkdownReply) -> Html {
    let value: String = reply.clone().into();
    let desc = md_string(&value);
    html! {
        <div class="entry markdown">{ desc }</div>
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
            Self::EntityObservation(entity) => Some(entity_observation(&entity)),
            Self::MarkdownReply(value) => Some(markdown_reply(&value)),

            Self::EditorReply(_) => None,
            Self::JsonReply(_) => todo!(),

            Self::CarryingEvent(event) => event.render(myself),
            Self::MovingEvent(event) => event.render(myself),
            Self::TalkingEvent(event) => event.render(myself),
        }
    }
}

#[derive(Properties, Clone, PartialEq)]
pub struct Props {
    pub entry: HistoryEntityPtr,
}

#[function_component(HistoryEntityPtrItem)]
pub fn history_entry_item(props: &Props) -> Html {
    let user = use_user_context();
    let key = user.key().expect("expected authenticated user with key");
    let myself = Myself {
        key: Some(key.clone()),
    };

    let value = &props.entry.value;
    match serde_json::from_value::<AllKnownItems>(value.clone()) {
        Ok(item) => match item.render(&myself) {
            Some(html) => html,
            None => html! {},
        },
        Err(e) => {
            log::warn!("{:?}", e);
            log::warn!("{:?}", value);

            html! {
                <div class="entry unknown">
                    { value.to_string() }
                </div>
            }
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
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            CarryingEvent::Held {
                living,
                item,
                area: _,
            } => Some(
                html! { <div class="entry"> { subject(living) } { " picked up " } { thing(item) }</div> },
            ),
            CarryingEvent::Dropped {
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
