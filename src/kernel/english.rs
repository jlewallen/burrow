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

#[derive(Debug, Clone, PartialEq)]
pub enum English {
    Literal(String),
    Phrase(Box<Vec<English>>),
    OneOf(Box<Vec<English>>),
    Optional(Box<English>),
    Unheld,
    Held,
    Contained,
    Numbered(u64),
    Text,
}

pub fn to_english(i: &str) -> IResult<&str, Vec<English>> {
    separated_list0(spaces, term)(i)
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
                    Some(v) => English::Phrase(Box::new(v.to_vec())),
                    None => todo!(),
                }
            } else {
                English::OneOf(Box::new(
                    optionals
                        .into_iter()
                        .map(|e| English::Phrase(Box::new(e)))
                        .collect(),
                ))
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

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        text: &'static str,
        expected: Vec<English>,
    }

    impl Fixture {
        pub fn new(text: &'static str, expected: Vec<English>) -> Self {
            Self { text, expected }
        }
    }

    fn get_fixtures() -> Vec<Fixture> {
        vec![
            Fixture::new(
                r#"HOLD #unheld"#,
                vec![English::Literal("HOLD".into()), English::Unheld],
            ),
            Fixture::new(
                r#"PUT #held IN #held"#,
                vec![
                    English::Literal("PUT".into()),
                    English::Held,
                    English::Literal("IN".into()),
                    English::Held,
                ],
            ),
            Fixture::new(
                r#"PUT #held (INSIDE OF|IN) (#held|#unheld)?"#,
                vec![
                    English::Literal("PUT".into()),
                    English::Held,
                    English::OneOf(Box::new(vec![
                        English::Phrase(Box::new(vec![
                            English::Literal("INSIDE".into()),
                            English::Literal("OF".into()),
                        ])),
                        English::Phrase(Box::new(vec![English::Literal("IN".into())])),
                    ])),
                    English::Optional(Box::new(English::OneOf(Box::new(vec![
                        English::Phrase(Box::new(vec![English::Held])),
                        English::Phrase(Box::new(vec![English::Unheld])),
                    ])))),
                ],
            ),
            Fixture::new(
                r#"TAKE (OUT)? #contained (OUT OF (#held|#unheld))?"#,
                vec![
                    English::Literal("TAKE".into()),
                    English::Optional(Box::new(English::Phrase(Box::new(vec![English::Literal(
                        "OUT".into(),
                    )])))),
                    English::Contained,
                    English::Optional(Box::new(English::Phrase(Box::new(vec![
                        English::Literal("OUT".into()),
                        English::Literal("OF".into()),
                        English::OneOf(Box::new(vec![
                            English::Phrase(Box::new(vec![English::Held])),
                            English::Phrase(Box::new(vec![English::Unheld])),
                        ])),
                    ])))),
                ],
            ),
            Fixture::new(
                r#"HOLD #unheld"#,
                vec![English::Literal("HOLD".into()), English::Unheld],
            ),
            Fixture::new(
                r#"DROP (#held)?"#,
                vec![
                    English::Literal("DROP".into()),
                    English::Optional(Box::new(English::Phrase(Box::new(vec![English::Held])))),
                ],
            ),
            Fixture::new(
                r#"EDIT #3493"#,
                vec![English::Literal("EDIT".into()), English::Numbered(3493)],
            ),
            Fixture::new(
                r#"DIG #text TO #text FOR #text"#,
                vec![
                    English::Literal("DIG".into()),
                    English::Text,
                    English::Literal("TO".into()),
                    English::Text,
                    English::Literal("FOR".into()),
                    English::Text,
                ],
            ),
            Fixture::new(
                r#"MAKE ITEM #text"#,
                vec![
                    English::Literal("MAKE".into()),
                    English::Literal("ITEM".into()),
                    English::Text,
                ],
            ),
        ]
    }

    #[test]
    fn should_parse_all_english_fixtures() {
        for fixture in get_fixtures() {
            let (remaining, actual) = to_english(fixture.text).unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, fixture.expected);
        }
    }

    #[test]
    fn should_transform_parser_for_english() {
        let english = vec![
            English::Literal("DROP".into()),
            // English::Literal("EVERYTHING".into()),
            English::Held,
        ];
        let mut parser = english_nodes_to_parser(&english);
        let value = parser("drop key").unwrap();
        println!("{:?}", value);
    }
}

#[allow(dead_code)]
#[derive(Debug)]
enum Node {
    Ignored,
    Held(String),
    Unheld(String),
    Contained(String),
    Phrase(Box<Vec<Node>>),
}

#[allow(dead_code)]
fn word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

#[allow(dead_code)]
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

#[allow(dead_code)]
fn english_nodes_to_parser<'a>(
    nodes: &'a Vec<English>,
) -> impl FnMut(&'a str) -> IResult<&'a str, Node> {
    move |mut i: &'a str| {
        // TODO Would love to move this up and out of the closure.
        let terms = nodes
            .into_iter()
            .map(english_node_to_parser)
            .collect::<Vec<_>>();

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

        Ok((i, Node::Phrase(Box::new(accumulator))))
    }
}
