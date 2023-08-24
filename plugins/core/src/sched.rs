use crate::library::plugin::*;

#[derive(Default)]
pub struct SchedulingPluginFactory {}

impl PluginFactory for SchedulingPluginFactory {
    fn create_plugin(&self) -> Result<Box<dyn Plugin>> {
        Ok(Box::new(SchedulingPlugin {}))
    }

    fn stop(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Default)]
pub struct SchedulingPlugin {}

impl Plugin for SchedulingPlugin {
    fn plugin_key() -> &'static str
    where
        Self: Sized,
    {
        "scheduling"
    }

    fn schema(&self) -> Schema {
        Schema::empty()
            .action::<actions::ScheduleAction>()
            .action::<actions::AddCronAction>()
            .action::<actions::RefreshCronAction>()
    }

    fn key(&self) -> &'static str {
        Self::plugin_key()
    }

    fn sources(&self) -> Vec<Box<dyn ActionSource>> {
        vec![Box::new(ActionSources::default())]
    }
}

impl ParsesActions for SchedulingPlugin {
    fn try_parse_action(&self, _i: &str) -> EvaluationResult {
        Err(EvaluationError::ParseFailed)
    }
}

#[derive(Default)]
pub struct ActionSources {}

impl ActionSource for ActionSources {
    fn try_deserialize_action(
        &self,
        tagged: &TaggedJson,
    ) -> Result<Option<Box<dyn Action>>, serde_json::Error> {
        try_deserialize_all!(
            tagged,
            actions::ScheduleAction,
            actions::AddCronAction,
            actions::RefreshCronAction
        );

        Ok(None)
    }
}

pub mod model {
    pub use crate::library::model::*;
    use std::str::FromStr;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct CronEntry {
        pub key: String,
        pub entity: EntityKey,
        pub spec: String,
        pub action: TaggedJson,
    }

    impl CronEntry {
        pub fn after(&self, now: &DateTime<Utc>) -> Option<DateTime<Utc>> {
            match cron::Schedule::from_str(&self.spec) {
                Ok(schedule) => schedule.after(now).take(1).next(),
                Err(e) => {
                    warn!("Cron error: {:?}", e);
                    None
                }
            }
        }

        pub fn queued(&self, now: &DateTime<Utc>) -> Option<FutureAction> {
            self.after(now).map(|time| {
                FutureAction::new(
                    self.key.clone(),
                    self.entity.clone(),
                    time.clone(),
                    self.action.clone(),
                )
            })
        }
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct CronTab {
        entries: Vec<CronEntry>,
    }

    impl CronTab {
        pub fn push(&mut self, entry: CronEntry) {
            self.entries.push(entry);
        }

        pub fn queued(&self, now: &DateTime<Utc>) -> Vec<FutureAction> {
            self.entries.iter().flat_map(|e| e.queued(now)).collect()
        }
    }

    impl Scope for CronTab {
        fn scope_key() -> &'static str {
            "crontab"
        }
    }
}

pub mod actions {
    use chrono::Duration;

    use super::model::*;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum ScheduleTime {
        Utc(DateTime<Utc>),
        Delay(i64),
    }

    impl Into<DateTime<Utc>> for ScheduleTime {
        fn into(self) -> DateTime<Utc> {
            match self {
                ScheduleTime::Utc(utc) => utc,
                ScheduleTime::Delay(millis) => Utc::now()
                    .checked_add_signed(Duration::milliseconds(millis))
                    .unwrap(),
            }
        }
    }

    #[action]
    pub struct ScheduleAction {
        pub key: String,
        pub entity: EntityKey,
        pub time: ScheduleTime,
        pub action: TaggedJson,
    }

    impl Action for ScheduleAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            let destined = FutureAction::new(
                self.key.clone(),
                self.entity.clone(),
                self.time.clone().into(),
                self.action.clone(),
            );

            session.schedule(destined)?;

            Ok(Effect::Ok)
        }
    }

    #[action]
    pub struct AddCronAction {
        pub key: String,
        pub entity: EntityKey,
        pub spec: String,
        pub action: TaggedJson,
    }

    impl Action for AddCronAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let world = surroundings.world();

            let mut tab = world.scope_mut::<CronTab>()?;
            let entry = CronEntry {
                key: self.key.clone(),
                entity: self.entity.clone(),
                spec: self.spec.clone(),
                action: self.action.clone(),
            };
            let queued = entry.queued(&Utc::now());
            tab.push(entry);
            tab.save()?;

            if let Some(queued) = queued {
                session.schedule(queued)?;
            }

            Ok(Effect::Ok)
        }
    }

    #[action]
    pub struct RefreshCronAction {
        pub now: DateTime<Utc>,
    }

    impl Action for RefreshCronAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, surroundings: &Surroundings) -> ReplyResult {
            let world = surroundings.world();

            if let Some(tab) = world.scope::<CronTab>()? {
                for queued in tab.queued(&self.now) {
                    session.schedule(queued)?;
                }
            }

            Ok(Effect::Ok)
        }
    }
}

pub mod parser {
    // use super::actions::*;
    // use crate::library::parser::*;
}

#[cfg(test)]
mod tests {
    // use super::model::*;
    // use super::parser::*;
    // use crate::library::tests::*;
}
