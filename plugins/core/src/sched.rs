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

pub mod model {
    pub use crate::library::model::*;
}

pub mod actions {
    use super::model::*;

    #[action]
    pub struct ScheduleAction {
        pub key: String,
        pub entity: EntityKey,
        pub time: DateTime<Utc>,
        pub message: TaggedJson,
    }

    impl Action for ScheduleAction {
        fn is_read_only() -> bool {
            true
        }

        fn perform(&self, session: SessionRef, _surroundings: &Surroundings) -> ReplyResult {
            let destined = FutureAction::new(
                self.key.clone(),
                self.entity.clone(),
                self.time,
                self.message.clone(),
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
