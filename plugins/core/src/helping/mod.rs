use crate::library::plugin::*;

#[cfg(test)]
mod tests;

#[derive(Default)]
pub struct HelpingPluginFactory {}

impl PluginFactory for HelpingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(HelpingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct HelpingPlugin {}

impl Plugin for HelpingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "helping"
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

impl ParsesActions for HelpingPlugin {
    fn try_parse_action(&self, i: &str) -> EvaluationResult {
        try_parsing(parser::HelpWithParser {}, i)
            .or_else(|_| try_parsing(parser::ReadHelpParser {}, i))
    }
}

pub mod model {
    use crate::library::model::*;

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Page {
        pub body: String,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct Wiki {
        // pub help: Option<HashMap<String, Page>>,
        pub acls: Option<Acls>,
        pub body: Option<String>,
    }

    // const DEFAULT_HELP: &str = "default";

    impl Wiki {
        pub fn get_default(&self) -> Option<String> {
            /*
            self.help
                .as_ref()
                .and_then(|h| h.get(DEFAULT_HELP).map(|h| h.body.to_owned()))
            */
            self.body.clone()
        }

        pub fn set_default(&mut self, value: &str) {
            /*
            let help = self.help.get_or_insert_with(|| HashMap::default());
            help.insert(
                DEFAULT_HELP.to_owned(),
                Page {
                    body: value.to_owned(),
                },
            );
            */
            self.body = Some(value.to_owned());
        }
    }

    impl Scope for Wiki {
        fn serialize(&self) -> Result<JsonValue> {
            Ok(serde_json::to_value(self)?)
        }

        fn scope_key() -> &'static str {
            "encyclopedia"
        }
    }

    impl Needs<SessionRef> for Wiki {
        fn supply(&mut self, _session: &SessionRef) -> Result<()> {
            Ok(())
        }
    }
}

pub mod actions {
    use engine::prelude::HasWellKnownEntities;

    use super::model::*;
    use crate::library::actions::*;

    fn lookup_page_name(
        session: &SessionRef,
        world: &Entry,
        page_name: Option<&str>,
        create: bool,
    ) -> Result<Option<Entry>, DomainError> {
        let Some(cyclo) = world.get_encyclopedia()? else {
                return Ok(None);
            };

        let cyclo = session.entry(&LookupBy::Key(&cyclo))?;
        let cyclo = cyclo.expect("TODO Dangling entity");

        if let Some(page_name) = page_name {
            let found = cyclo.get_well_known_by_name(page_name)?;
            if let Some(found) = found {
                Ok(session.entry(&LookupBy::Key(&found))?)
            } else {
                if create {
                    let creating: Entity = build_entity()
                        .class(EntityClass::encyclopedia())
                        .name(page_name)
                        .try_into()?;
                    let creating = session.add_entity(&EntityPtr::new(creating))?;
                    let mut wiki = creating.scope_mut::<Wiki>()?;
                    wiki.set_default("# Hello, world!");
                    wiki.save()?;
                    Ok(Some(creating))
                } else {
                    Ok(None)
                }
            }
        } else {
            Ok(Some(cyclo))
        }
    }

    #[action]
    pub struct ReadHelpAction {
        pub page_name: Option<String>,
    }

    impl Action for ReadHelpAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let (world, _, _) = surroundings.unpack();

            let page = lookup_page_name(
                &session,
                &world,
                self.page_name.as_ref().map(|s| s.as_str()),
                false,
            )?;
            let Some(page) = page else {
                return Ok(SimpleReply::NotFound.try_into()?)
            };

            let wiki = page.scope::<Wiki>()?;
            let reply: MarkdownReply = wiki.get_default().unwrap_or_else(|| "".to_owned()).into();
            Ok(reply.try_into()?)
        }
    }

    #[action]
    pub struct EditHelpAction {
        pub page_name: Option<String>,
    }

    impl Action for EditHelpAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("editing {:?}", self.page_name);

            let (world, _, _) = surroundings.unpack();
            let page = lookup_page_name(
                &session,
                &world,
                self.page_name.as_ref().map(|s| s.as_str()),
                true,
            )?;
            let Some(page) = page else {
                return Ok(SimpleReply::NotFound.try_into()?);
            };

            let wiki = page.scope::<Wiki>()?;
            let body: String = wiki.get_default().unwrap_or_else(|| "".to_owned()).into();
            let reply = EditorReply::new(
                page.key().to_string(),
                WorkingCopy::Markdown(body),
                SaveHelpAction::new_template(page.key().clone())?,
            );
            Ok(reply.try_into()?)
        }
    }

    #[action]
    pub struct SaveHelpAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
    }

    impl SaveHelpAction {
        pub fn new(key: EntityKey, copy: WorkingCopy) -> Self {
            Self { key, copy }
        }

        pub fn new_template(key: EntityKey) -> Result<JsonTemplate, TaggedJsonError> {
            let copy = WorkingCopy::Markdown(JSON_TEMPLATE_VALUE_SENTINEL.to_owned());
            let template = Self { key, copy };

            Ok(template.to_tagged_json()?.into())
        }
    }

    impl Action for SaveHelpAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            info!("saving {:?}", self.key);

            match session.entry(&LookupBy::Key(&self.key))? {
                Some(entry) => {
                    match &self.copy {
                        WorkingCopy::Markdown(markdown) => {
                            let mut wiki = entry.scope_mut::<Wiki>()?;
                            wiki.set_default(markdown);
                            wiki.save()?;
                        }
                        _ => unimplemented!(),
                    }

                    Ok(SimpleReply::Done.try_into()?)
                }
                None => Ok(SimpleReply::NotFound.try_into()?),
            }
        }
    }
}

pub mod parser {
    use super::actions::*;
    use crate::library::parser::*;

    pub struct ReadHelpParser {}

    impl ParsesActions for ReadHelpParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                pair(tag("help"), opt(preceded(spaces, text_to_end_of_line))),
                |(_, page_name)| {
                    Box::new(ReadHelpAction {
                        page_name: page_name.map(|n| n.to_owned()),
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }

    pub struct HelpWithParser {}

    impl ParsesActions for HelpWithParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                pair(
                    tuple((tag("edit"), spaces, tag("help"))),
                    opt(preceded(spaces, text_to_end_of_line)),
                ),
                |(_, page_name)| {
                    Box::new(EditHelpAction {
                        page_name: page_name.map(|n| n.to_owned()),
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }
}
