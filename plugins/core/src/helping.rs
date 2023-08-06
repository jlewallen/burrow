use crate::library::plugin::*;

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
        fn serialize(&self) -> Result<serde_json::Value> {
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
    use engine::HasWellKnownEntities;

    use super::model::*;
    use crate::library::actions::*;

    fn lookup_page_name(
        session: &SessionRef,
        world: &Entry,
        page_name: Option<&str>,
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
                todo!("Create new pages")
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
            )?;
            let Some(page) = page else {
                return Ok(Effect::Reply(EffectReply::Instance(Rc::new(SimpleReply::NotFound))));
            };

            let wiki = page.scope::<Wiki>()?;
            let reply: MarkdownReply = wiki.get_default().unwrap_or_else(|| "".to_owned()).into();
            Ok(Effect::Reply(EffectReply::Instance(Rc::new(reply))))
        }
    }

    #[action]
    pub struct HelpWithAction {
        pub page_name: String,
    }

    impl Action for HelpWithAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            info!("editing {:?}", self.page_name);

            let (world, _, _) = surroundings.unpack();
            let page = lookup_page_name(&session, &world, Some(&self.page_name))?;
            let Some(page) = page else {
                return Ok(Effect::Reply(EffectReply::Instance(Rc::new(SimpleReply::NotFound))));
            };

            let wiki = page.scope::<Wiki>()?;
            let body: String = wiki.get_default().unwrap_or_else(|| "".to_owned()).into();
            let reply = EditorReply::new(page.key().to_string(), WorkingCopy::Markdown(body));
            Ok(Effect::Reply(EffectReply::Instance(Rc::new(reply))))
        }
    }

    #[action]
    pub struct SaveHelpAction {
        pub key: EntityKey,
        pub copy: WorkingCopy,
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

                    Ok(SimpleReply::Done.into())
                }
                None => Ok(SimpleReply::NotFound.into()),
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
            let (_, action) = map(alt((tag("help"), tag("wtf"))), |_s| {
                Box::new(ReadHelpAction { page_name: None }) as Box<dyn Action>
            })(i)?;

            Ok(Some(action))
        }
    }

    pub struct HelpWithParser {}

    impl ParsesActions for HelpWithParser {
        fn try_parse_action(&self, i: &str) -> EvaluationResult {
            let (_, action) = map(
                separated_pair(
                    separated_pair(tag("help"), spaces, tag("with")),
                    spaces,
                    text_to_end_of_line,
                ),
                |(_, page_name)| {
                    Box::new(HelpWithAction {
                        page_name: page_name.to_owned(),
                    }) as Box<dyn Action>
                },
            )(i)?;

            Ok(Some(action))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parser::*;
    use crate::library::tests::*;

    #[test]
    fn it_reads_default_help_on_help() -> Result<()> {
        let (_surroundings, effect) = parse_and_perform(ReadHelpParser {}, "help")?;

        assert!(matches!(effect, Effect::Ok));

        Ok(())
    }

    #[test]
    fn it_reads_default_help_on_wtf() -> Result<()> {
        let (_surroundings, effect) = parse_and_perform(ReadHelpParser {}, "wtf")?;

        assert!(matches!(effect, Effect::Ok));

        Ok(())
    }

    #[test]
    fn it_allows_helping_with_help() -> Result<()> {
        let (_surroundings, effect) = parse_and_perform(HelpWithParser {}, "help with Food")?;

        assert!(matches!(effect, Effect::Ok));

        Ok(())
    }

    /*
    #[test]
    fn it_requires_help_with_page() -> Result<()> {
        let err = parse_and_perform(HelpWithParser {}, "help with");

        assert_eq!(err.unwrap_err(), EvaluationError::ParseFailed);

        Ok(())
    }
    */
}
