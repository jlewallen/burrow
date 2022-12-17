use crate::plugins::library::plugin::*;

pub struct BuildingPlugin {}

impl ParsesActions for BuildingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::EditActionParser {}, i)
            .or_else(|_| try_parsing(parser::BidirectionalDigActionParser {}, i))
            .or_else(|_| try_parsing(parser::MakeItemParser {}, i))
    }
}

pub mod model {
    use crate::plugins::library::model::*;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    struct EditorReply {}

    impl Reply for EditorReply {}

    impl ToJson for EditorReply {
        fn to_json(&self) -> Result<Value, serde_json::Error> {
            Ok(serde_json::to_value(self)?)
        }
    }

    pub fn discover(_source: &Entry, _entity_keys: &mut [EntityKey]) -> Result<()> {
        Ok(())
    }
}

pub mod actions {
    use crate::plugins::{library::actions::*, looking::actions::LookAction};

    #[derive(Debug)]
    pub struct EditAction {
        pub item: Item,
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
    pub struct BidirectionalDigAction {
        pub outgoing: String,
        pub returning: String,
        pub new_area: String,
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

            let (_, living, area, infra) = args.clone();

            let new_area = EntityPtr::new_named(&self.new_area, &self.new_area)?;
            let returning = EntityPtr::new_named(&self.returning, &self.returning)?;
            let outgoing = EntityPtr::new_named(&self.outgoing, &self.outgoing)?;
            let added = infra.add_entities(&vec![&new_area, &returning, &outgoing])?;
            let new_area = added[0].clone();
            let returning = added[1].clone();
            let outgoing = added[2].clone();

            tools::leads_to(&returning, &area)?;
            tools::set_container(&new_area, &vec![returning.clone()])?;

            tools::leads_to(&outgoing, &new_area)?;
            tools::set_container(&area, &vec![outgoing.clone()])?;

            // TODO Chain to GoAction?
            match tools::navigate_between(&area, &new_area, &living)? {
                DomainOutcome::Ok => infra.chain(&living, Box::new(LookAction {})),
                DomainOutcome::Nope => Ok(Box::new(SimpleReply::NotFound)),
            }
        }
    }

    #[derive(Debug)]
    pub struct MakeItemAction {
        pub name: String,
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

            tools::set_container(&user, &vec![new_item.try_into()?])?;

            Ok(Box::new(SimpleReply::Done))
        }
    }
}

pub mod parser {
    use crate::plugins::library::parser::*;

    use super::actions::{BidirectionalDigAction, EditAction, MakeItemAction};

    pub struct MakeItemParser {}

    impl ParsesActions for MakeItemParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                tuple((preceded(
                    pair(separated_pair(tag("make"), spaces, tag("item")), spaces),
                    string_literal,
                ),)),
                |name| MakeItemAction {
                    name: name.0.into(),
                },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct EditActionParser {}

    impl ParsesActions for EditActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                preceded(pair(tag("edit"), spaces), noun_or_specific),
                |item| EditAction { item },
            )(i)?;

            Ok(Box::new(action))
        }
    }

    pub struct BidirectionalDigActionParser {}

    impl ParsesActions for BidirectionalDigActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                tuple((
                    preceded(pair(tag("dig"), spaces), string_literal),
                    preceded(pair(spaces, pair(tag("to"), spaces)), string_literal),
                    preceded(pair(spaces, pair(tag("for"), spaces)), string_literal),
                )),
                |(outgoing, returning, new_area)| BidirectionalDigAction {
                    outgoing: outgoing.into(),
                    returning: returning.into(),
                    new_area: new_area.into(),
                },
            )(i)?;

            Ok(Box::new(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use super::*;
    use crate::{
        domain::{BuildActionArgs, QuickThing},
        plugins::{carrying::model::Containing, looking::model::new_area_observation},
    };

    #[test]
    fn it_fails_to_edit_unknown_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(EditActionParser {}, "edit rake")?;
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

        let action = try_parsing(EditActionParser {}, "edit broom")?;
        let reply = action.perform(args.clone())?;

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        Ok(())
    }

    #[test]
    fn it_fails_to_edit_items_by_missing_gid() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(EditActionParser {}, "edit #1201")?;
        let reply = action.perform(args.clone())?;

        assert_eq!(reply.to_json()?, SimpleReply::NotFound.to_json()?);

        Ok(())
    }

    #[test]
    fn it_edits_items_by_gid() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build
            .ground(vec![QuickThing::Object("Cool Broom")])
            .try_into()?;

        let action = try_parsing(EditActionParser {}, "edit #1")?;
        let reply = action.perform(args.clone())?;

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        Ok(())
    }

    #[test]
    fn it_digs_bidirectionally() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build.plain().try_into()?;

        let action = try_parsing(
            BidirectionalDigActionParser {},
            r#"dig "North Exit" to "South Exit" for "New Area""#,
        )?;
        let reply = action.perform(args.clone())?;
        let (_, living, _area, infra) = args.clone();

        // Not the best way of finding the constructed area.
        let destination = infra
            .load_entity_by_gid(&EntityGID::new(7))?
            .ok_or(DomainError::EntityNotFound)?;

        assert_eq!(
            reply.to_json()?,
            new_area_observation(&living, &destination.try_into()?)?.to_json()?
        );

        Ok(())
    }

    #[test]
    fn it_makes_items() -> Result<()> {
        let mut build = BuildActionArgs::new()?;
        let args: ActionArgs = build.plain().try_into()?;

        let action = try_parsing(MakeItemParser {}, r#"make item "Blue Rake""#)?;
        let reply = action.perform(args.clone())?;
        let (_, living, _area, _infra) = args.clone();

        assert_eq!(reply.to_json()?, SimpleReply::Done.to_json()?);

        assert_eq!(living.scope::<Containing>()?.holding.len(), 1);

        Ok(())
    }
}
