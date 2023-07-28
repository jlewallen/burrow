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

    pub fn string_literal(i: &str) -> IResult<&str, &str> {
        delimited(tag("\""), string_inside, tag("\""))(i)
    }

    fn string_inside(i: &str) -> IResult<&str, &str> {
        take_while(|c: char| c.is_alphabetic() || c.is_whitespace())(i)
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
        alt((surrounding_area, noun, gid_reference))(i)
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
    pub use kernel::*;
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

    pub fn reply_done<T: DomainEvent + 'static>(
        audience: Audience,
        raise: T,
    ) -> Result<SimpleReply> {
        get_my_session()?.raise(audience, Box::new(raise))?;

        Ok(SimpleReply::Done)
    }
}

pub mod tests {
    pub use crate::{BuildSurroundings, QuickThing};
    pub use kernel::{DomainError, EntityGid, LookupBy, SimpleReply};
}
