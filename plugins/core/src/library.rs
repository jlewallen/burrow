pub mod plugin {
    pub use anyhow::Result;
    pub use serde::Deserialize;
    pub use tracing::*;

    pub use super::parser::try_parsing;
    pub use kernel::common::DeserializeTagged;
    pub use kernel::prelude::*;
    pub use kernel::try_deserialize_all;
}

pub mod parser {
    pub use nom::{
        branch::alt,
        bytes::complete::{tag, take_while, take_while1},
        character::complete::char,
        character::complete::digit1,
        character::complete::one_of,
        combinator::map,
        combinator::{map_res, opt, recognize},
        multi::{many0, many1},
        sequence::delimited,
        sequence::{pair, preceded, separated_pair, terminated, tuple},
        IResult,
    };
    pub use tracing::*;

    pub use kernel::prelude::*;

    fn decimal(i: &str) -> IResult<&str, &str> {
        recognize(many1(terminated(one_of("0123456789"), many0(char('_')))))(i)
    }

    // This was copied from the nom recipes and was the fastest solution I could
    // find to being able to parse f64 and i64 separately.
    fn float(i: &str) -> IResult<&str, &str> {
        alt((
            // Case one: .42
            recognize(tuple((
                char('.'),
                decimal,
                opt(tuple((one_of("eE"), opt(one_of("+-")), decimal))),
            ))), // Case two: 42e42 and 42.42e42
            recognize(tuple((
                decimal,
                opt(preceded(char('.'), decimal)),
                one_of("eE"),
                opt(one_of("+-")),
                decimal,
            ))), // Case three: 42. and 42.42
            recognize(tuple((decimal, char('.'), opt(decimal)))),
        ))(i)
    }

    fn parse_quantity(i: &str) -> IResult<&str, Quantity> {
        let float = map(float, |s: &str| {
            Quantity::Fractional(s.parse::<f64>().expect("Error parsing fractional quantity"))
        });

        let integer = map(decimal, |s: &str| {
            Quantity::Whole(s.parse::<i64>().expect("Error parsing whole quantity"))
        });

        alt((float, integer))(i)
    }

    pub fn spaces(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| " \t".contains(c))(i)
    }

    pub fn noun(i: &str) -> IResult<&str, Item> {
        map(word, |s: &str| Item::Named(s.to_owned()))(i)
    }

    pub fn person(i: &str) -> IResult<&str, Item> {
        map(word, |s: &str| Item::Named(s.to_owned()))(i)
    }

    pub fn text_to_end_of_line(i: &str) -> IResult<&str, &str> {
        take_while1(move |_| true)(i)
    }

    fn string_inside(i: &str) -> IResult<&str, &str> {
        take_while(move |c: char| c.is_alphabetic() || c.is_whitespace())(i)
    }

    pub fn string_literal(i: &str) -> IResult<&str, &str> {
        delimited(tag("\""), string_inside, tag("\""))(i)
    }

    const LOWER_CASE_CHARS: &str = "abcdefghijklmnopqrstuvwxyz";
    const LETTER_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

    pub fn word(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| LETTER_CHARS.contains(c))(i)
    }

    pub fn camel_case_word(i: &str) -> IResult<&str, &str> {
        recognize(pair(
            take_while1(move |c| LOWER_CASE_CHARS.contains(c)),
            many0(take_while1(move |c| LETTER_CHARS.contains(c))),
        ))(i)
    }

    fn unsigned_number(i: &str) -> IResult<&str, u64> {
        map_res(recognize(digit1), str::parse)(i)
    }

    const KEY_CHARS: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0-9";

    fn key(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| KEY_CHARS.contains(c))(i)
    }

    fn key_reference(i: &str) -> IResult<&str, Item> {
        map(preceded(tag("~"), key), |n| Item::Key(EntityKey::new(n)))(i)
    }

    pub fn gid_reference(i: &str) -> IResult<&str, Item> {
        map(preceded(tag("#"), unsigned_number), |n| {
            Item::Gid(EntityGid::new(n))
        })(i)
    }

    fn surrounding_area(i: &str) -> IResult<&str, Item> {
        map(alt((tag("area"), tag("here"))), |_s: &str| Item::Area)(i)
    }

    pub fn quantified(i: &str) -> IResult<&str, Item> {
        map(separated_pair(parse_quantity, spaces, noun), |(q, n)| {
            Item::Quantified(q, n.into())
        })(i)
    }

    pub fn noun_or_specific(i: &str) -> IResult<&str, Item> {
        alt((
            surrounding_area,
            myself,
            quantified,
            noun,
            gid_reference,
            key_reference,
        ))(i)
    }

    pub fn myself(i: &str) -> IResult<&str, Item> {
        map(alt((tag("me"), tag("myself"), tag("self"))), |_s: &str| {
            Item::Myself
        })(i)
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

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        pub fn test_camel_case() {
            assert_eq!(camel_case_word("hello").unwrap(), ("", "hello"));
            assert_eq!(camel_case_word("helloWorld").unwrap(), ("", "helloWorld"));
            assert_eq!(camel_case_word("hello_").unwrap(), ("_", "hello"));
        }

        #[test]
        pub fn test_quantified() {
            assert_eq!(
                quantified("10 coins").unwrap(),
                (
                    "",
                    Item::Quantified(Quantity::Whole(10), Item::Named("coins".to_owned()).into())
                )
            );

            assert_eq!(
                quantified("1.5 liters").unwrap(),
                (
                    "",
                    Item::Quantified(
                        Quantity::Fractional(1.5),
                        Item::Named("liters".to_owned()).into()
                    )
                )
            );
        }
    }
}

pub mod model {
    pub use anyhow::Result;
    pub use chrono::{DateTime, Utc};
    pub use kernel::common::*;
    pub use kernel::prelude::*;
    pub use macros::*;
    pub use serde::{Deserialize, Serialize};
    pub use serde_json::json;
    pub use std::rc::Rc;
    pub use std::{collections::HashMap, ops::Deref};
    pub use tracing::*;
}

pub mod actions {
    pub use crate::tools;
    pub use anyhow::Result;
    pub use kernel::common::*;
    pub use kernel::prelude::*;
    pub use macros::*;
    pub use serde::{Deserialize, Serialize};
    pub use tracing::*;

    pub fn reply_ok<T: ToTaggedJson + 'static>(
        actor: EntityPtr,
        audience: Audience,
        raise: T,
    ) -> Result<Effect> {
        get_my_session()?.raise(
            Some(actor),
            audience,
            Raising::TaggedJson(raise.to_tagged_json()?),
        )?;

        Ok(Effect::Ok)
    }

    pub fn reply_done<T: ToTaggedJson + 'static>(
        actor: EntityPtr,
        audience: Audience,
        raise: T,
    ) -> Result<SimpleReply> {
        get_my_session()?.raise(
            Some(actor),
            audience,
            Raising::TaggedJson(raise.to_tagged_json()?),
        )?;

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
    pub use kernel::common::*;
    pub use kernel::here;
    pub use kernel::prelude::*;
    pub use tracing::*;

    pub use super::plugin::try_parsing;

    pub trait ToDebugJson {
        fn to_debug_json(&self) -> Result<JsonValue, serde_json::Error>;
    }

    impl ToDebugJson for Effect {
        fn to_debug_json(&self) -> Result<JsonValue, serde_json::Error> {
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

    pub fn perform_directly<T: Action>(action: T) -> Result<(Surroundings, Effect)> {
        let mut build = BuildSurroundings::new()?;
        let (session, surroundings) = build.plain().encyclopedia()?.build()?;
        let action: Rc<dyn Action> = Rc::new(action);
        let effect = action.perform(session.clone(), &surroundings)?;
        build.close()?;
        Ok((surroundings, effect))
    }
}
