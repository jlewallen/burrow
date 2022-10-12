use anyhow::Result;
// use tracing::{debug, info};

use nom::{
    branch::alt,
    bytes::complete::{/*is_not,*/ tag, take_while1},
    // character::complete::{alpha1, char},
    combinator::map,
    // error::{context, ContextError, ParseError},
    sequence::separated_pair,
    IResult,
};

pub trait Node: std::fmt::Debug {
    fn describe(&self) -> String;
}

// Maybe we define separate parsers for each language?
#[derive(Debug)]
pub struct English {
    n: Box<dyn Node>,
}

fn word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

fn spaces(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| " \t".contains(c))(i)
}

#[derive(Debug)]
struct NounNode {
    value: String,
}

impl Node for NounNode {
    fn describe(&self) -> String {
        self.value.to_owned()
    }
}

fn noun(i: &str) -> IResult<&str, Box<dyn Node>> {
    map(word, |s: &str| -> Box<dyn Node> {
        Box::new(NounNode {
            value: s.to_owned(),
        })
    })(i)
}

#[derive(Debug)]
struct LookNode {}

impl Node for LookNode {
    fn describe(&self) -> String {
        "look".to_string()
    }
}

fn look(i: &str) -> IResult<&str, Box<dyn Node>> {
    map(tag("look"), |_| -> Box<dyn Node> { Box::new(LookNode {}) })(i)
}

#[derive(Debug)]
struct HoldNode {
    target: Box<dyn Node>,
}

impl Node for HoldNode {
    fn describe(&self) -> String {
        format!("hold {}", self.target.describe())
    }
}

fn hold(i: &str) -> IResult<&str, Box<dyn Node>> {
    map(
        separated_pair(tag("hold"), spaces, noun),
        |(_, target)| -> Box<dyn Node> { Box::new(HoldNode { target: target }) },
    )(i)
}

#[derive(Debug)]
struct DropNode {
    target: Option<Box<dyn Node>>,
}

impl Node for DropNode {
    fn describe(&self) -> String {
        match &self.target {
            Some(item) => format!("drop {}", item.describe()),
            None => format!("drop"),
        }
    }
}

fn drop(i: &str) -> IResult<&str, Box<dyn Node>> {
    let specific = map(
        separated_pair(tag("drop"), spaces, noun),
        |(_, target)| -> Box<dyn Node> {
            Box::new(DropNode {
                target: Some(target),
            })
        },
    );

    let everything = map(tag("drop"), |_| -> Box<dyn Node> {
        Box::new(DropNode { target: None })
    });

    alt((specific, everything))(i)
}

impl English {
    pub fn parse(s: &str) -> IResult<&str, Self> {
        let ours = alt((look, hold, drop));

        map(ours, |node| Self { n: node })(s)
    }

    pub fn describe(&self) -> String {
        self.n.describe()
    }
}

pub fn evaluate() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_look_correctly() {
        let (remaining, actual) = English::parse("look").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual.describe(), "look")
    }

    #[test]
    fn it_parses_hold_noun_correctly() {
        let (remaining, actual) = English::parse("hold rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual.describe(), "hold rake")
    }
    #[test]
    fn it_parses_solo_drop_correctly() {
        let (remaining, actual) = English::parse("drop").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual.describe(), "drop")
    }
    #[test]
    fn it_parses_drop_noun_correctly() {
        let (remaining, actual) = English::parse("drop rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual.describe(), "drop rake")
    }
}
