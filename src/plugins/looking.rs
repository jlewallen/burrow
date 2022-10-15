use crate::kernel::*;
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
        Err(_e) => Err(EvaluationError::ParseFailed),
    }
}

pub mod model {
    use crate::kernel::*;
    use anyhow::Result;

    pub fn discover(_source: &Entity, _entity_keys: &mut Vec<EntityKey>) -> Result<()> {
        Ok(())
    }

    #[derive(Debug)]
    pub struct LookReply {}

    impl Reply for LookReply {
        fn to_markdown(&self) -> Result<Markdown> {
            let mut md = Markdown::new(Vec::new());
            md.write("")?;
            Ok(md)
        }
    }
}

pub mod actions {
    use super::model::*;
    use super::*;
    use anyhow::Result;
    use tracing::info;

    #[derive(Debug)]
    struct LookAction {}
    impl Action for LookAction {
        fn perform(&self, (_world, _user, _area): ActionArgs) -> Result<Box<dyn Reply>> {
            info!("look!");

            Ok(Box::new(LookReply {}))
        }
    }

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
