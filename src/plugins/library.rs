use crate::kernel::Item;
use nom::{bytes::complete::take_while1, combinator::map, IResult};

pub fn word(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
}

pub fn spaces(i: &str) -> IResult<&str, &str> {
    take_while1(move |c| " \t".contains(c))(i)
}

pub fn noun(i: &str) -> IResult<&str, Item> {
    map(word, |s: &str| Item::Named(s.to_owned()))(i)
}

pub fn named_place(i: &str) -> IResult<&str, Item> {
    map(word, |s: &str| Item::Route(s.to_owned()))(i)
}
