pub mod model {
    use crate::plugins::library::model::*;

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
    use super::parser::{parse, Sentence};
    use crate::plugins::library::actions::*;

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

    pub fn evaluate(i: &str) -> EvaluationResult {
        Ok(parse(i).map(|(_, sentence)| evaluate_sentence(&sentence))?)
    }

    fn evaluate_sentence(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Edit(e) => Box::new(EditAction {
                maybe_item: e.clone(),
            }),
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Edit(Item),
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        edit_item(i)
    }

    fn edit_item(i: &str) -> IResult<&str, Sentence> {
        map(separated_pair(tag("edit"), spaces, noun), |(_, target)| {
            Sentence::Edit(target)
        })(i)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_edit_noun_correctly() {
            let (remaining, actual) = parse("edit rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Edit(Item::Named("rake".to_owned())));
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let actual = parse("hello");
            assert!(actual.is_err());
        }
    }
}
