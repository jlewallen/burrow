pub use nom::{
    branch::alt,
    bytes::complete::{tag, take_while, take_while1},
    character::complete::digit1,
    combinator::{map, map_res, opt, recognize},
    multi::separated_list0,
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
use nom::{bytes::complete::tag_no_case, combinator::eof};
pub use tracing::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq)]
pub enum English {
    Literal(String),
    Phrase(Vec<English>),
    OneOf(Vec<English>),
    Optional(Box<English>),
    Unheld,
    Held,
    Contained,
    Numbered(u64),
    Text,
}

pub fn to_english(text: &str) -> IResult<&str, Vec<English>> {
    separated_list0(spaces, term)(text)
}

pub fn to_tongue(text: &str) -> Option<Vec<English>> {
    match to_english(text) {
        Ok((_, e)) => Some(e),
        Err(_) => None,
    }
}

fn term(i: &str) -> IResult<&str, English> {
    alt((
        literal,
        contained,
        unheld,
        held,
        text,
        numbered,
        optional_phrase,
    ))(i)
}

fn optional_phrase(i: &str) -> IResult<&str, English> {
    map(
        tuple((phrase, opt(tag("?")))),
        |(p, is_optional)| match is_optional {
            Some(_) => English::Optional(Box::new(p)),
            None => p,
        },
    )(i)
}

fn phrase(i: &str) -> IResult<&str, English> {
    map(
        delimited(
            tag("("),
            separated_list0(tag("|"), separated_list0(spaces, term)),
            tag(")"),
        ),
        |optionals| {
            if optionals.len() == 1 {
                match optionals.get(0) {
                    Some(v) => English::Phrase(v.to_vec()),
                    None => todo!(),
                }
            } else {
                English::OneOf(optionals.into_iter().map(|e| English::Phrase(e)).collect())
            }
        },
    )(i)
}

fn spaces(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| " \t".contains(c))(i)
}

fn unsigned_number(i: &str) -> IResult<&str, u64> {
    map_res(recognize(digit1), str::parse)(i)
}

fn numbered(i: &str) -> IResult<&str, English> {
    map(preceded(tag("#"), unsigned_number), |n| {
        English::Numbered(n)
    })(i)
}

fn unheld(i: &str) -> IResult<&str, English> {
    map(tag("#unheld"), |_| English::Unheld)(i)
}

fn held(i: &str) -> IResult<&str, English> {
    map(tag("#held"), |_| English::Held)(i)
}

fn contained(i: &str) -> IResult<&str, English> {
    map(tag("#contained"), |_| English::Contained)(i)
}

fn text(i: &str) -> IResult<&str, English> {
    map(tag("#text"), |_| English::Text)(i)
}

fn uppercase_word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "ABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

fn literal(i: &str) -> IResult<&str, English> {
    map(uppercase_word, |v| English::Literal(v.into()))(i)
}

fn word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

fn english_node_to_parser<'a>(
    node: &'a English,
) -> Box<dyn FnMut(&'a str) -> IResult<&'a str, Node> + 'a> {
    match node {
        English::Literal(v) => Box::new(map(tag_no_case::<&str, &str, _>(v), |_| Node::Ignored)),
        English::Phrase(_) => todo!(),
        English::OneOf(_) => todo!(),
        English::Optional(_) => todo!(),
        English::Unheld => Box::new(map(word, |w| Node::Unheld(w.into()))),
        English::Held => Box::new(map(word, |w| Node::Held(w.into()))),
        English::Contained => Box::new(map(word, |w| Node::Contained(w.into()))),
        English::Numbered(_) => todo!(),
        English::Text => todo!(),
    }
}

fn english_nodes_to_parser<'a>(
    nodes: &'a [English],
) -> impl FnMut(&'a str) -> IResult<&'a str, Node> {
    move |mut i: &'a str| {
        // TODO Would love to move this up and out of the closure.
        let terms = nodes.iter().map(english_node_to_parser).collect::<Vec<_>>();

        let mut accumulator: Vec<Node> = vec![];
        for mut term in terms {
            let (r, term_node) = term(i)?;
            let (r, _) = alt((spaces, eof))(r)?;
            i = r;

            match term_node {
                Node::Ignored => {}
                _ => accumulator.push(term_node),
            }
        }

        Ok((i, Node::Phrase(accumulator)))
    }
}

#[derive(Debug)]
pub enum Node {
    Ignored,
    Held(String),
    Unheld(String),
    Contained(String),
    Phrase(Vec<Node>),
}

pub fn try_parse(english: &[English], text: &str) -> Option<Node> {
    match english_nodes_to_parser(english)(text) {
        Ok((_, node)) => Some(node),
        Err(_) => None,
    }
}
