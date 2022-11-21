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
    use crate::plugins::{library::actions::*, looking::actions::LookAction};

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

    #[derive(Debug)]
    struct BidirectionalDigAction {
        outgoing: String,
        returning: String,
        new_area: String,
    }

    impl Action for BidirectionalDigAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!(
                "bidirectional-dig {:?} <-> {:?} '{:?}'",
                self.outgoing, self.returning, self.new_area
            );

            let (_, user, area, infra) = args.clone();

            let new_area = EntityPtr::new_named(&self.new_area, &self.new_area)?;
            let returning = EntityPtr::new_named(&self.returning, &self.returning)?;
            tools::leads_to(&returning, &area)?;
            tools::set_container(&new_area, &vec![returning.clone()])?;

            let outgoing = EntityPtr::new_named(&self.outgoing, &self.outgoing)?;
            tools::leads_to(&outgoing, &new_area)?;
            tools::set_container(&area, &vec![outgoing.clone()])?;

            infra.add_entities(&vec![&new_area, &returning, &outgoing])?;

            info!("entity {:?} {:?} {:?}", outgoing, returning, new_area);

            match tools::navigate_between(&area, &new_area, &user)? {
                DomainOutcome::Ok(_) => infra.chain(&user, Box::new(LookAction {})),
                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    struct MakeItemAction {
        name: String,
    }

    impl Action for MakeItemAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, args: ActionArgs) -> ReplyResult {
            info!("make-item {:?}", self.name);

            let (_, user, _area, infra) = args.clone();

            let new_item = EntityPtr::new_named(&self.name, &self.name)?;

            infra.add_entities(&vec![&new_item])?;

            tools::set_container(&user, &vec![new_item])?;

            Ok(Box::new(SimpleReply::Done))
        }
    }

    pub struct BuildingPlugin {}

    impl ParsesActions for BuildingPlugin {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            evaluate(i)
        }
    }

    fn evaluate(i: &str) -> EvaluationResult {
        Ok(parse(i).map(|(_, sentence)| evaluate_sentence(&sentence))?)
    }

    fn evaluate_sentence(s: &Sentence) -> Box<dyn Action> {
        match s {
            Sentence::Edit(e) => Box::new(EditAction { item: e.clone() }),
            Sentence::BidirectionalDig(outgoing, returning, new_area) => {
                Box::new(BidirectionalDigAction {
                    outgoing: outgoing.clone(),
                    returning: returning.clone(),
                    new_area: new_area.clone(),
                })
            }
            Sentence::MakeItem(name) => Box::new(MakeItemAction { name: name.clone() }),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{
            domain::{BuildActionArgs, QuickThing},
            plugins::{carrying::model::Containing, looking::model::AreaObservation},
        };

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

        #[test]
        fn it_digs_bidirectionally() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build.plain().try_into()?;

            let action = BidirectionalDigAction {
                outgoing: "North Exit".into(),
                returning: "South Exit".into(),
                new_area: "New Area".into(),
            };
            let reply = action.perform(args.clone())?;
            let (_, living, _area, infra) = args.clone();

            // Not the best way of finding the constructed area.
            let destination = infra
                .load_entity_by_gid(&EntityGID::new(4))?
                .ok_or(DomainError::EntityNotFound)?;

            assert_eq!(
                reply.to_json()?,
                AreaObservation::new(&living, &destination)?.to_json()?
            );

            Ok(())
        }

        #[test]
        fn it_makes_items() -> Result<()> {
            let mut build = BuildActionArgs::new()?;
            let args: ActionArgs = build.plain().try_into()?;

            let action = MakeItemAction {
                name: "Blue Rake".into(),
            };
            let reply = action.perform(args.clone())?;
            let (_, living, _area, _infra) = args.clone();

            assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

            assert_eq!(living.borrow().scope::<Containing>()?.holding.len(), 1);

            Ok(())
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    #[derive(Debug, Clone, Eq, PartialEq)]
    pub enum Sentence {
        Edit(Item),
        BidirectionalDig(String, String, String),
        MakeItem(String),
    }

    pub fn parse(i: &str) -> IResult<&str, Sentence> {
        alt((edit_item, make_item, dig_bidirectional_routes_to_new_area))(i)
    }

    fn make_item(i: &str) -> IResult<&str, Sentence> {
        map(
            tuple((preceded(
                pair(separated_pair(tag("make"), spaces, tag("item")), spaces),
                string_literal,
            ),)),
            |name| Sentence::MakeItem(name.0.into()),
        )(i)
    }

    fn dig_bidirectional_routes_to_new_area(i: &str) -> IResult<&str, Sentence> {
        map(
            tuple((
                preceded(pair(tag("dig"), spaces), string_literal),
                preceded(pair(spaces, pair(tag("to"), spaces)), string_literal),
                preceded(pair(spaces, pair(tag("for"), spaces)), string_literal),
            )),
            |(outgoing, returning, new_area)| {
                Sentence::BidirectionalDig(outgoing.into(), returning.into(), new_area.into())
            },
        )(i)
    }

    fn edit_item(i: &str) -> IResult<&str, Sentence> {
        map(
            preceded(pair(tag("edit"), spaces), noun_or_specific),
            Sentence::Edit,
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
        fn it_parses_digging_north_south_to_new_area() {
            let (remaining, _actual) =
                parse(r#"dig "NORTH EXIT" to "SOUTH EXIT" for "A NEW AREA""#).unwrap();
            assert_eq!(remaining, "");
        }

        #[test]
        fn it_parses_make_item() {
            let (remaining, _actual) = parse(r#"make item "COOL RAKE""#).unwrap();
            assert_eq!(remaining, "");
        }

        #[test]
        fn it_errors_on_unknown_text() {
            let actual = parse("hello");
            assert!(actual.is_err());
        }
    }
}
