pub mod plugin {
    pub use super::parser::{try_parsing, ParsesActions};
    pub use crate::kernel::*;
}

pub mod parser {
    pub use crate::kernel::*;
    use nom::sequence::delimited;
    pub use nom::{
        branch::alt,
        bytes::complete::{tag, take_while, take_while1},
        character::complete::digit1,
        combinator::map,
        combinator::{map_res, recognize},
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

    fn string_inside(i: &str) -> IResult<&str, &str> {
        take_while(|c: char| c.is_alphabetic() || c.is_whitespace())(i)
    }

    pub fn string_literal(i: &str) -> IResult<&str, &str> {
        delimited(tag("\""), string_inside, tag("\""))(i)
    }

    pub fn unsigned_number(i: &str) -> IResult<&str, u64> {
        map_res(recognize(digit1), str::parse)(i)
    }

    pub fn gid_reference(i: &str) -> IResult<&str, Item> {
        map(preceded(tag("#"), unsigned_number), |n| {
            Item::GID(EntityGID::new(n))
        })(i)
    }

    pub fn noun_or_specific(i: &str) -> IResult<&str, Item> {
        alt((noun, gid_reference))(i)
    }

    pub fn named_place(i: &str) -> IResult<&str, Item> {
        alt((
            gid_reference,
            map(word, |s: &str| Item::Route(s.to_owned())),
        ))(i)
    }

    pub trait ParsesActions {
        fn try_parse_action(&self, i: &str) -> EvaluationResult;
    }

    pub fn try_parsing<T: ParsesActions>(parser: T, i: &str) -> EvaluationResult {
        parser.try_parse_action(i)
    }
}

pub mod model {
    pub use crate::kernel::*;
    pub use anyhow::Result;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::{json, Value};
    pub use std::{collections::HashMap, ops::Deref};
    pub use tracing::*;

    pub trait DiscoversEntities {
        fn discover_entities(
            &self,
            source: &Entity,
            entity_keys: &mut Vec<EntityKey>,
        ) -> Result<()>;
    }
}

pub mod actions {
    pub use crate::kernel::*;
    pub use crate::plugins::library::parser::ParsesActions;
    pub use crate::plugins::log_test;
    pub use crate::plugins::tools;
    pub use anyhow::Result;
    pub use tracing::*;
}
