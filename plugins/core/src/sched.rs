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
        Schema::empty().action::<actions::ScheduleAction>()
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
        try_deserialize_all!(tagged, actions::ScheduleAction);

        Ok(None)
    }
}

pub mod actions {
    use std::ops::Add;

    pub use crate::library::actions::*;
    pub use crate::library::model::*;
    use chrono::Duration;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum ScheduleTime {
        Utc(DateTime<Utc>),
        Delay(i64),
        Cron(String),
    }

    impl HasArgumentType for ScheduleTime {
        fn argument_type() -> ArgumentType {
            ArgumentType::Time
        }
    }

    impl Into<FutureSchedule> for ScheduleTime {
        fn into(self) -> FutureSchedule {
            match self {
                ScheduleTime::Utc(time) => FutureSchedule::Utc(time),
                ScheduleTime::Cron(spec) => FutureSchedule::Cron(spec),
                ScheduleTime::Delay(millis) => {
                    FutureSchedule::Utc(Utc::now().add(Duration::milliseconds(millis)))
                }
            }
        }
    }

    #[action]
    pub struct ScheduleAction {
        pub key: String,
        pub actor: EntityKey,
        pub schedule: ScheduleTime,
        pub action: TaggedJson,
    }

    impl Action for ScheduleAction {
        fn is_read_only() -> bool {
            false
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            let destined = FutureAction::new(
                self.key.clone(),
                self.actor.clone(),
                self.schedule.clone().into(),
                self.action.clone(),
            );

            session.schedule(destined)?;

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
