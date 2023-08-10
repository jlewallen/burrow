pub mod plugin {
    pub use super::parser::{try_parsing, ParsesActions};
    pub use anyhow::Result;
    pub use kernel::*;
    pub use tracing::*;
}

pub mod parser {
    pub use kernel::*;
    use nom::sequence::delimited;
    pub use nom::{
        branch::alt,
        bytes::complete::{tag, take_while, take_while1},
        character::complete::digit1,
        combinator::map,
        combinator::{map_res, opt, recognize},
        sequence::{pair, preceded, separated_pair, tuple},
        IResult,
    };
    pub use tracing::*;

    pub fn word(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
    }

    pub fn spaces(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| " \t".contains(c))(i)
    }

    pub fn noun(i: &str) -> IResult<&str, Item> {
        map(word, |s: &str| Item::Named(s.to_owned()))(i)
    }

    pub fn text_to_end_of_line(i: &str) -> IResult<&str, &str> {
        take_while1(|_c: char| true)(i)
    }

    pub fn string_literal(i: &str) -> IResult<&str, &str> {
        delimited(tag("\""), string_inside, tag("\""))(i)
    }

    fn string_inside(i: &str) -> IResult<&str, &str> {
        take_while(|c: char| c.is_alphabetic() || c.is_whitespace())(i)
    }

    pub fn camel_case_word(i: &str) -> IResult<&str, &str> {
        word(i) // TODO
    }

    pub fn unsigned_number(i: &str) -> IResult<&str, u64> {
        map_res(recognize(digit1), str::parse)(i)
    }

    pub fn gid_reference(i: &str) -> IResult<&str, Item> {
        map(preceded(tag("#"), unsigned_number), |n| {
            Item::Gid(EntityGid::new(n))
        })(i)
    }

    pub fn surrounding_area(i: &str) -> IResult<&str, Item> {
        map(alt((tag("area"), tag("here"))), |_s: &str| Item::Area)(i)
    }

    pub fn noun_or_specific(i: &str) -> IResult<&str, Item> {
        alt((surrounding_area, myself, noun, gid_reference))(i)
    }

    pub fn myself(i: &str) -> IResult<&str, Item> {
        map(alt((tag("self"), tag("myself"))), |_s: &str| Item::Myself)(i)
    }

    pub fn named_place(i: &str) -> IResult<&str, Item> {
        alt((
            gid_reference,
            map(word, |s: &str| Item::Route(s.to_owned())),
        ))(i)
    }

    pub fn try_parsing<T: ParsesActions>(parser: T, i: &str) -> EvaluationResult {
        parser.try_parse_action(i)
    }
}

pub mod model {
    pub use anyhow::Result;
    pub use chrono::{DateTime, Utc};
    pub use kernel::*;
    pub use macros::*;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{json, Value};
    pub use std::rc::Rc;
    pub use std::{collections::HashMap, ops::Deref};
    pub use tracing::*;
}

pub mod actions {
    pub use crate::library::parser::ParsesActions;
    pub use crate::tools;
    pub use anyhow::Result;
    pub use kernel::*;
    pub use macros::*;
    pub use serde::{Deserialize, Serialize};
    pub use tracing::*;

    pub fn reply_ok<T: ToJson + 'static>(audience: Audience, raise: T) -> Result<Effect> {
        get_my_session()?.raise(audience, Raising::TaggedJson(raise.to_tagged_json()?))?;

        Ok(Effect::Ok)
    }

    pub fn reply_done<T: ToJson + 'static>(audience: Audience, raise: T) -> Result<SimpleReply> {
        get_my_session()?.raise(audience, Raising::TaggedJson(raise.to_tagged_json()?))?;

        Ok(SimpleReply::Done)
    }
}

pub mod tests {
    pub use anyhow::Result;
    pub use chrono::TimeZone;
    pub use chrono::Utc;
    pub use serde::de::DeserializeOwned;
    pub use std::collections::HashSet;
    pub use std::rc::Rc;

    pub use crate::tools;
    pub use crate::{BuildSurroundings, QuickThing};
    pub use kernel::*;
    pub use tracing::*;

    pub use super::plugin::try_parsing;

    pub trait ToDebugJson {
        fn to_debug_json(&self) -> Result<serde_json::Value, serde_json::Error>;
    }

    impl ToDebugJson for Effect {
        fn to_debug_json(&self) -> Result<serde_json::Value, serde_json::Error> {
            serde_json::to_value(self)
        }
    }

    pub fn parse_and_perform<T: ParsesActions>(
        parser: T,
        line: &str,
    ) -> Result<(Surroundings, Effect)> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().encyclopedia()?.build()?;
        let action = try_parsing(parser, line)?;
        let action = action.unwrap();
        let effect = action.perform(session.clone(), &surroundings)?;
        build.close()?;
        Ok((surroundings, effect))
    }
}
