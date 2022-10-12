use anyhow::Result;
// use tracing::{debug, info};

trait Action {
    fn perform(&self) -> Result<()>;
}

mod vocab {
    use nom::{
        branch::alt,
        bytes::complete::{/*is_not,*/ tag, take_while1},
        // character::complete::{alpha1, char},
        combinator::map,
        // error::{context, ContextError, ParseError},
        sequence::separated_pair,
        IResult,
    };

    use super::Action;
    use anyhow::Result;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Item {
        Described(String),
    }

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Look,
        Hold(Item),
        Drop(Option<Item>),
    }

    // Maybe we define separate parsers for each language?
    #[derive(Debug, Eq, PartialEq)]
    pub struct English {
        pub s: Sentence,
    }

    fn word(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c))(i)
    }

    fn spaces(i: &str) -> IResult<&str, &str> {
        take_while1(move |c| " \t".contains(c))(i)
    }

    fn noun(i: &str) -> IResult<&str, Item> {
        map(word, |s: &str| Item::Described(s.to_owned()))(i)
    }

    fn look(i: &str) -> IResult<&str, Sentence> {
        map(tag("look"), |_| Sentence::Look)(i)
    }

    fn hold(i: &str) -> IResult<&str, Sentence> {
        map(separated_pair(tag("hold"), spaces, noun), |(_, target)| {
            Sentence::Hold(target)
        })(i)
    }
    fn drop(i: &str) -> IResult<&str, Sentence> {
        let specific = map(separated_pair(tag("drop"), spaces, noun), |(_, target)| {
            Sentence::Drop(Some(target))
        });

        let everything = map(tag("drop"), |_| Sentence::Drop(None));

        alt((specific, everything))(i)
    }

    impl English {
        pub fn parse(s: &str) -> IResult<&str, Self> {
            let ours = alt((look, hold, drop));

            map(ours, |sentence| Self { s: sentence })(s)
        }
    }

    pub trait Visitor<T> {
        fn visit_sentence(&mut self, n: &Sentence) -> T;
    }

    struct LookAction {}
    impl Action for LookAction {
        fn perform(&self) -> Result<()> {
            Ok(())
        }
    }

    struct HoldAction {
        sentence: Sentence,
    }
    impl Action for HoldAction {
        fn perform(&self) -> Result<()> {
            Ok(())
        }
    }

    struct DropAction {
        sentence: Sentence,
    }
    impl Action for DropAction {
        fn perform(&self) -> Result<()> {
            Ok(())
        }
    }

    struct Interpreter;

    impl Visitor<Box<dyn Action>> for Interpreter {
        fn visit_sentence(&mut self, s: &Sentence) -> Box<dyn Action> {
            match *s {
                Sentence::Hold(ref e) => Box::new(HoldAction {
                    sentence: s.clone(), // TODO Another way to achieve this?
                }),
                Sentence::Drop(ref e) => Box::new(DropAction {
                    sentence: s.clone(), // TODO Another way to achieve this?
                }),
                Sentence::Look => Box::new(LookAction {}),
            }
        }
    }
}

pub fn evaluate(s: &str) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::vocab::*;

    #[test]
    fn it_parses_look_correctly() {
        let (remaining, actual) = English::parse("look").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, English { s: Sentence::Look })
    }

    #[test]
    fn it_parses_hold_noun_correctly() {
        let (remaining, actual) = English::parse("hold rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            actual,
            English {
                s: Sentence::Hold(Item::Described("rake".to_owned()))
            }
        )
    }
    #[test]
    fn it_parses_solo_drop_correctly() {
        let (remaining, actual) = English::parse("drop").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            actual,
            English {
                s: Sentence::Drop(None)
            }
        )
    }
    #[test]
    fn it_parses_drop_noun_correctly() {
        let (remaining, actual) = English::parse("drop rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(
            actual,
            English {
                s: Sentence::Drop(Some(Item::Described("rake".to_owned())))
            }
        )
    }
}
