use anyhow::Result;
use thiserror::Error;

pub trait Action {
    fn perform(&self) -> Result<()>;
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error("unknown parsing human readable")]
    ParseError,
}

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    looking::evaluate(i).or(carrying::evaluate(i))
}

mod library {
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
}

mod looking {
    // use super::library::*;
    use super::{Action, EvaluationError};
    use anyhow::Result;

    use nom::{bytes::complete::tag, combinator::map, IResult};

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Look,
    }

    fn look(i: &str) -> IResult<&str, Sentence> {
        map(tag("look"), |_| Sentence::Look)(i)
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        look(i)
    }

    pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
        match parse(i).map(|(_, sentence)| actions::evaluate(&sentence)) {
            Ok(action) => Ok(action),
            Err(_e) => Err(EvaluationError::ParseError), // TODO Weak
        }
    }

    mod actions {
        use super::super::Action;
        use super::*;

        use anyhow::Result;

        use tracing::info;

        struct LookAction {}
        impl Action for LookAction {
            fn perform(&self) -> Result<()> {
                info!("look!");

                Ok(())
            }
        }

        /*
        pub trait Visitor<T> {
            fn visit_sentence(&mut self, n: &Sentence) -> T;
        }

        struct Interpreter;

        impl Visitor<Box<dyn Action>> for Interpreter {
            fn visit_sentence(&mut self, s: &Sentence) -> Box<dyn Action> {
                match *s {
                    Sentence::Look => Box::new(LookAction {}),
                }
            }
        }
        */

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            match *s {
                Sentence::Look => Box::new(LookAction {}),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_look_correctly() {
            let (remaining, actual) = parse("look").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Look)
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err()); // TODO Weak
        }
    }
}

mod carrying {
    use super::library::*;
    use super::{Action, EvaluationError};

    use nom::{
        branch::alt, bytes::complete::tag, combinator::map, sequence::separated_pair, IResult,
    };

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Hold(Item),
        Drop(Option<Item>),
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

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        alt((hold, drop))(i)
    }

    pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
        match parse(i).map(|(_, sentence)| actions::evaluate(&sentence)) {
            Ok(action) => Ok(action),
            Err(_e) => Err(EvaluationError::ParseError), // TODO Weak
        }
    }

    mod actions {
        use super::super::Action;
        use super::*;

        use anyhow::Result;
        use tracing::info;

        struct HoldAction {
            sentence: Sentence,
        }
        impl Action for HoldAction {
            fn perform(&self) -> Result<()> {
                info!("hold {:?}!", self.sentence);

                Ok(())
            }
        }

        struct DropAction {
            sentence: Sentence,
        }
        impl Action for DropAction {
            fn perform(&self) -> Result<()> {
                info!("drop {:?}!", self.sentence);

                Ok(())
            }
        }

        pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
            // TODO This could be improved.
            match *s {
                Sentence::Hold(ref _e) => Box::new(HoldAction {
                    sentence: s.clone(),
                }),
                Sentence::Drop(ref _e) => Box::new(DropAction {
                    sentence: s.clone(),
                }),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_hold_noun_correctly() {
            let (remaining, actual) = parse("hold rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Hold(Item::Described("rake".to_owned())))
        }

        #[test]
        fn it_parses_solo_drop_correctly() {
            let (remaining, actual) = parse("drop").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Drop(None))
        }

        #[test]
        fn it_parses_drop_noun_correctly() {
            let (remaining, actual) = parse("drop rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(
                actual,
                Sentence::Drop(Some(Item::Described("rake".to_owned())))
            )
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let output = parse("hello");
            assert!(output.is_err()); // TODO Weak
        }
    }
}
