use crate::library::plugin::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct FashionPluginFactory {}

impl PluginFactory for FashionPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(FashionPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct FashionPlugin {}

impl Plugin for FashionPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "fashion"
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn middleware(&mut self) -> Result<Vec<Rc<dyn Middleware>>> {
        Ok(Vec::default())
    }

    fn deliver(&self, _incoming: &Incoming) -> Result<()> {
        Ok(())
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

impl ParsesActions for FashionPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::WearActionParser {}, i)
            .or_else(|_| try_parsing(parser::RemoveActionParser {}, i))
    }
}

pub mod model {
    use crate::library::model::*;

    pub type CarryingResult = Result<DomainOutcome>;

    #[derive(Debug, Serialize, ToJson)]
    #[serde(rename_all = "camelCase")]
    pub enum FashionEvent {
        Worn {
            living: EntityRef,
            item: ObservedEntity,
            area: EntityRef,
        },
        Removed {
            living: EntityRef,
            item: ObservedEntity,
            area: EntityRef,
        },
    }

    impl DomainEvent for FashionEvent {}

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub enum Article {
        Just(EntityRef),
    }

    impl Article {
        pub fn keys(&self) -> Vec<&EntityRef> {
            match self {
                Article::Just(e) => vec![e],
            }
        }

        pub fn to_entry(&self) -> Result<Entry, DomainError> {
            match self {
                Article::Just(e) => Ok(e.to_entry()?),
            }
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Wearing {
        pub wearing: Vec<Article>,
    }

    impl Scope for Wearing {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "wearing"
        }
    }

    impl Needs<SessionRef> for Wearing {
        fn supply(&mut self, session: &SessionRef) -> Result<()> {
            self.wearing = self
                .wearing
                .iter()
                .map(|r| match r {
                    Article::Just(e) => Ok(Article::Just(session.ensure_entity(e)?)),
                })
                .collect::<Result<Vec<_>, DomainError>>()?;

            Ok(())
        }
    }

    impl Wearing {
        pub fn start_wearing(&mut self, item: &Entry) -> Result<DomainOutcome, DomainError> {
            if !item.has_scope::<Wearable>()? {
                return Ok(DomainOutcome::Nope);
            }

            let wearable = item.scope::<Wearable>()?;

            let wearing = self
                .wearing
                .iter()
                .map(|h| match h {
                    Article::Just(e) => e.to_entry(),
                })
                .collect::<Result<Vec<_>, _>>()?;

            for held in wearing {
                if is_kind(&held, &wearable.kind)? {
                    return Ok(DomainOutcome::Ok);
                }
            }

            self.wearing.push(Article::Just(item.try_into()?));

            Ok(DomainOutcome::Ok)
        }

        pub fn is_wearing(&self, item: &Entry) -> Result<bool> {
            Ok(self
                .wearing
                .iter()
                .map(|i| i.keys())
                .flatten()
                .any(|i| *i.key() == *item.key()))
        }

        fn remove_item(&mut self, item: &Entry) -> CarryingResult {
            self.wearing = self
                .wearing
                .iter()
                .flat_map(|i| {
                    if i.keys().into_iter().any(|i| *i.key() == *item.key()) {
                        vec![]
                    } else {
                        vec![i.clone()]
                    }
                })
                .collect::<Vec<_>>()
                .to_vec();

            Ok(DomainOutcome::Ok)
        }

        pub fn stop_wearing(&mut self, item: &Entry) -> Result<Option<Entry>> {
            if !self.is_wearing(item)? {
                return Ok(None);
            }

            self.remove_item(item)?;

            Ok(Some(item.clone()))
        }
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Wearable {
        kind: Kind,
    }

    fn is_kind(entity: &Entry, kind: &Kind) -> Result<bool> {
        Ok(*entity.scope::<Wearable>()?.kind() == *kind)
    }

    impl Default for Wearable {
        fn default() -> Self {
            let session = get_my_session().expect("No session in Entity::new_blank!");
            Self {
                kind: Kind::new(session.new_identity()),
            }
        }
    }

    impl Wearable {
        pub fn kind(&self) -> &Kind {
            &self.kind
        }

        pub fn set_kind(&mut self, kind: &Kind) {
            self.kind = kind.clone();
        }
    }

    impl Scope for Wearable {
        fn serialize(&self) -> Result<serde_json::Value> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "wearable"
        }
    }

    impl Needs<SessionRef> for Wearable {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
        }
    }
}

pub mod actions {
    use super::model::*;
    use crate::{library::actions::*, looking::model::Observe};

    #[action]
    pub struct WearAction {
        pub item: Item,
    }

    impl Action for WearAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("wear {:?}!", self.item);

            let (_, user, area) = surroundings.unpack();

            match session.find_item(surroundings, &self.item)? {
                Some(holding) => match tools::move_between(&area, &user, &holding)? {
                    DomainOutcome::Ok => Ok(reply_done(
                        Audience::Area(area.key().clone()),
                        FashionEvent::Worn {
                            living: user.entity_ref(),
                            item: (&holding).observe(&user)?.expect("No observed entity"),
                            area: area.entity_ref(),
                        },
                    )?
                    .into()),
                    DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }

    #[action]
    pub struct RemoveAction {
        pub maybe_item: Option<Item>,
    }

    impl Action for RemoveAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("remove {:?}!", self.maybe_item);

            let (_, user, area) = surroundings.unpack();

            match &self.maybe_item {
                Some(item) => match session.find_item(surroundings, item)? {
                    Some(dropping) => match tools::move_between(&user, &area, &dropping)? {
                        DomainOutcome::Ok => Ok(reply_done(
                            Audience::Area(area.key().clone()),
                            FashionEvent::Removed {
                                living: user.entity_ref(),
                                item: (&dropping).observe(&user)?.expect("No observed entity"),
                                area: area.entity_ref(),
                            },
                        )?
                        .into()),
                        DomainOutcome::Nope => Ok(SimpleReply::NotFound.into()),
                    },
                    None => Ok(SimpleReply::NotFound.into()),
                },
                None => Ok(SimpleReply::NotFound.into()),
            }
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct WearActionParser {}

    impl ParsesActions for WearActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(separated_pair(tag("wear"), spaces, noun), |(_, target)| {
                WearAction { item: target }
            })(i)?;

            Ok(Some(Box::new(action)))
        }
    }

    pub struct RemoveActionParser {}

    impl ParsesActions for RemoveActionParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let specific = map(
                separated_pair(tag("remove"), spaces, noun),
                |(_, target)| RemoveAction {
                    maybe_item: Some(Item::Held(Box::new(target))),
                },
            );

            let everything = map(tag("remove"), |_| RemoveAction { maybe_item: None });

            let (_, action) = alt((specific, everything))(i)?;

            Ok(Some(Box::new(action)))
        }
    }
}
