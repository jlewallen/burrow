pub mod parser {
    pub use crate::kernel::*;
    use nom::sequence::preceded;
    pub use nom::{
        branch::alt,
        bytes::complete::{tag, take_while1},
        character::complete::digit1,
        combinator::map,
        combinator::{map_res, recognize},
        sequence::separated_pair,
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

    pub fn number(i: &str) -> IResult<&str, i64> {
        map_res(recognize(digit1), str::parse)(i)
    }

    pub fn gid_reference(i: &str) -> IResult<&str, Item> {
        map(preceded(tag("#"), number), |n| Item::GID(EntityGID::new(n)))(i)
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
}

pub mod model {
    pub use crate::kernel::*;
    pub use anyhow::Result;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::Value;
    pub use std::collections::HashMap;
    pub use std::ops::Deref;
    pub use tracing::*;
}

pub mod actions {
    pub use crate::kernel::*;
    pub use crate::plugins::tools;
    pub use anyhow::Result;
    pub use tracing::*;
}
