pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EditorReply {}

    impl Reply for EditorReply {}

    impl ToJson for EditorReply {
        fn to_json(&self) -> Result<Value> {
            Ok(serde_json::to_value(self)?)
        }
    }

    pub fn discover(_source: &Entity, _entity_keys: &mut [EntityKey]) -> Result<()> {
        Ok(())
    }
}

pub mod actions {
    use super::parser::{parse, Sentence};
    use crate::plugins::library::actions::*;

    #[derive(Debug)]
    struct EditAction {
        item: Item,
    }

    impl Action for EditAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("editing {:?}!", self.item);

            let (_, _, _, infra) = args.clone();

            match infra.find_item(args, &self.item)? {
                Some(editing) => {
                    info!("editing {:?}", editing);
                    Ok(Box::new(SimpleReply::Done))
                }
                None => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    pub fn evaluate(i: &str) -> EvaluationResult {
        Ok(parse(i).map(|(_, sentence)| evaluate_sentence(&sentence))?)
    }

    fn evaluate_sentence(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Edit(e) => Box::new(EditAction { item: e.clone() }),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::domain::{BuildActionArgs, QuickThing};

        #[test]
        fn it_fails_to_edit_unknown_items() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Broom")])
                .try_into()?;

            let action = EditAction {
                item: Item::Named("rake".into()),
            };
            let reply = action.perform(args.clone())?;
            // let (_, _, _, _) = args.clone();

            assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

            Ok(())
        }

        #[test]
        fn it_edits_items_named() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Broom")])
                .try_into()?;

            let action = EditAction {
                item: Item::Named("broom".into()),
            };
            let reply = action.perform(args.clone())?;

            assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

            Ok(())
        }

        #[test]
        fn it_edits_items_by_gid() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Broom")])
                .try_into()?;

            let action = EditAction {
                item: Item::GID(EntityGID::new(1201)),
            };
            let reply = action.perform(args.clone())?;

            assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

            Ok(())
        }

        #[test]
        fn it_fails_to_edit_items_by_missing_gid() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build
                .ground(vec![QuickThing::Object("Cool Broom")])
                .try_into()?;

            let action = EditAction {
                item: Item::GID(EntityGID::new(1)),
            };
            let reply = action.perform(args.clone())?;

            assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

            Ok(())
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
        map(
            separated_pair(tag("edit"), spaces, noun_or_specific),
            |(_, target)| Sentence::Edit(target),
        )(i)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn it_parses_edit_noun_correctly() {
            let (remaining, actual) = parse("edit rake").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Edit(Item::Named("rake".into())));
        }

        #[test]
        fn it_parses_edit_gid_number_correctly() {
            let (remaining, actual) = parse("edit #608").unwrap();
            assert_eq!(remaining, "");
            assert_eq!(actual, Sentence::Edit(Item::GID(EntityGID::new(608))));
        }

        #[test]
        fn it_skips_parsing_misleading_gid_number() {
            let (remaining, _actual) = parse("edit #3g34").unwrap();
            assert_eq!(remaining, "g34");
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let actual = parse("hello");
            assert!(actual.is_err());
        }
    }
}
