use std::collections::HashMap;

use regex::Captures;
use replies::*;
use yew::prelude::*;

use crate::{
    hooks::use_user_context,
    types::{AllKnownItems, Diagnostics, Entry, HistoryEntityPtr, Myself, Run},
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

            Self::Carrying(event) => event.render(myself),
            Self::Moving(event) => event.render(myself),
            Self::Talking(event) => event.render(myself),

            Self::Diagnostics(diagnostics) => diagnostics.render(myself),
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

impl Render for Carrying {
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            Carrying::Held {
                actor,
                item,
                area: _,
            } => Some(
                html! { <div class="entry"> { subject(actor) } { " picked up " } { thing(item) }</div> },
            ),
            Carrying::Dropped {
                actor,
                item,
                area: _,
            } => Some(
                html! { <div class="entry"> { subject(actor) } { " dropped " } { thing(item) }</div> },
            ),
        }
    }
}

impl Render for Moving {
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            Moving::Left { actor, area: _ } => {
                Some(html! { <div class="entry"> { subject(actor) } { " left." } </div> })
            }
            Moving::Arrived { actor, area: _ } => {
                Some(html! { <div class="entry"> { subject(actor) } { " arrived." } </div> })
            }
        }
    }
}

impl Render for Talking {
    fn render(&self, _myself: &Myself) -> Option<Html> {
        match self {
            Talking::Conversation(s) => Some(
                html! { <div class="entry"> <span class="speaker">{ s.who.name.as_ref().unwrap() }</span>{ ": " } { &s.message } </div> },
            ),
            Talking::Whispering(_) => todo!(),
        }
    }
}

impl Render for Diagnostics {
    fn render(&self, myself: &Myself) -> Option<Html> {
        Some(html! {
            <div class="entry diagnostics">
                { self.runs.iter().flat_map(|i| i.render(myself)).collect::<Html>() }
            </div>
        })
    }
}

impl Render for JsonValue {
    fn render(&self, myself: &Myself) -> Option<Html> {
        match self {
            JsonValue::Null => Some(html! { "<null>" }),
            JsonValue::Bool(b) => Some(html! { b }),
            JsonValue::Number(value) => Some(html! { value }),
            JsonValue::String(value) => Some(html! { value }),
            JsonValue::Array(values) => Some(html! {
                <>{ values.iter().flat_map(|value| value.render(myself)).collect::<Html>() }</>
            }),
            JsonValue::Object(obj) => Some(html! {
                obj.iter().map(|(key, value)| {
                    html! { <> <span class="key">{ key }{ ": " }</span>{ value.render(myself) } </> }
                }).collect::<Html>()
            }),
        }
    }
}

impl Render for Run {
    fn render(&self, myself: &Myself) -> Option<Html> {
        fn span(span: &HashMap<String, JsonValue>, myself: &Myself) -> Html {
            html! {
                <span class="span"> {
                    span.iter().map(|(key, value)| {
                        html! { <> <span class="key">{ key }{ ": " }</span>{ value.render(myself) } </> }
                    }).collect::<Html>()
                } </span>
            }
        }

        fn entry(entry: &Entry, myself: &Myself) -> Html {
            html! {
                <div class="log-entry">
                    <span class={classes!("level", &entry.level)}>{ &entry.level }</span>
                    <span class="spans">{ entry.spans.iter().map(|s| span(s, myself)).collect::<Html>() }</span>
                    <span class="target">{ &entry.target }</span>
                    <span class="message">{ &entry.fields.message }</span>
                    if !entry.fields.extra.is_empty() {
                        <span class="extra">{ format!("{:?}", &entry.fields.extra) }</span>
                    }
                </div>
            }
        }

        match self {
            Run::Diagnostics {
                time: _,
                desc,
                logs,
            } => Some(html! {
                <div class="run">
                    <h4>{ &desc }</h4>

                    <div class="logs">
                        { logs.iter().map(|l| entry(&l, myself)).collect::<Html>() }
                    </div>
                </div>
            }),
        }
    }
}
