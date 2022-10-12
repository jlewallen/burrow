use nom::{bytes::complete::take_while1, combinator::map, IResult};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Item {
    Described(String),
}

pub fn word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

pub fn spaces(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| " \t".contains(c))(i)
}

pub fn noun(i: &str) -> IResult<&str, Item> {
    map(word, |s: &str| Item::Described(s.to_owned()))(i)
}
