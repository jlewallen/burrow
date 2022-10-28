use anyhow::Result;
use nom::{bytes::complete::tag, combinator::map, sequence::separated_pair, IResult};

use super::library::{noun, spaces};
use crate::kernel::*;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Sentence {
    Edit(Item),
}

fn edit_item(i: &str) -> IResult<&str, Sentence> {
    map(separated_pair(tag("edit"), spaces, noun), |(_, target)| {
        Sentence::Edit(target)
    })(i)
}

pub fn parse(i: &str) -> IResult<&str, Sentence> {
    edit_item(i)
}

pub fn evaluate(i: &str) -> Result<Box<dyn Action>, EvaluationError> {
    Ok(parse(i).map(|(_, sentence)| actions::evaluate(&sentence))?)
}

pub mod model {
    use anyhow::Result;
    use serde::Serialize;
    use serde_json::Value;

    use crate::kernel::*;

    pub fn discover(_source: &Entity, _entity_keys: &mut [EntityKey]) -> Result<()> {
        Ok(())
    }

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EditorReply {}

    impl Reply for EditorReply {
        fn to_markdown(&self) -> Result<Markdown> {
            let mut md = Markdown::new(Vec::new());
            md.write("")?;
            Ok(md)
        }
    }

    impl ToJson for EditorReply {
        fn to_json(&self) -> Result<Value> {
            Ok(serde_json::to_value(self)?)
        }
    }
}

pub mod actions {
    use tracing::info;

    use super::*;

    #[derive(Debug)]
    struct EditAction {
        maybe_item: Item,
    }
    impl Action for EditAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, (_world, _user, _area, _infra): ActionArgs) -> ReplyResult {
            info!("edit {:?}!", self.maybe_item);

            Ok(Box::new(SimpleReply::Done))
        }
    }

    pub fn evaluate(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Edit(e) => Box::new(EditAction {
                maybe_item: e.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_parses_edit_noun_correctly() {
        let (remaining, actual) = parse("edit rake").unwrap();
        assert_eq!(remaining, "");
        assert_eq!(actual, Sentence::Edit(Item::Named("rake".to_owned())))
    }

    #[test]
    fn it_errors_on_unknown_text() {
        let output = parse("hello");
        assert!(output.is_err()); // TODO Weak assertion.
    }
}
