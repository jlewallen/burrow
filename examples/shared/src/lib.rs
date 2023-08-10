use anyhow::Result;
use chrono::Duration;
use tracing::*;

use dynlib_sys::prelude::*;
use macros::*;
use plugins_core::library::model::*;

#[derive(Debug, Serialize, Deserialize, ToTaggedJson)]
enum ExampleFuture {
    Wakeup,
}

impl TryInto<ExampleFuture> for Incoming {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<ExampleFuture, Self::Error> {
        Ok(serde_json::from_value(self.value.into_tagged())?)
    }
}

#[derive(Default)]
struct ExampleAgent {}

impl Agent for ExampleAgent {
    fn initialize(&mut self) -> Result<()> {
        if false {
            get_my_session()?.schedule(
                "example-test",
                When::Interval(Duration::minutes(1)),
                &ExampleFuture::Wakeup,
            )?;
        }

        Ok(())
    }

    fn have_surroundings(&mut self, _surroundings: Surroundings) -> Result<()> {
        Ok(())
    }

    fn deliver(&mut self, incoming: Incoming) -> Result<()> {
        let incoming: ExampleFuture = incoming.try_into()?;
        info!("{:?}", incoming);

        match incoming {
            ExampleFuture::Wakeup => Ok(()),
        }
    }
}

#[derive(Debug, Serialize, ToTaggedJson)]
struct ExampleReply {}

impl Reply for ExampleReply {}

dynlib_sys::export_plugin!(agent_initialize, agent_middleware, agent_tick);

#[allow(improper_ctypes_definitions)]
extern "C" fn agent_initialize(dh: &mut dyn DynamicHost) {
    default_agent_initialize::<ExampleAgent>(dh);
}

#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn agent_tick(dh: &mut dyn DynamicHost, state: *const std::ffi::c_void) {
    default_agent_tick::<ExampleAgent>(dh, state);
}

#[allow(improper_ctypes_definitions)]
unsafe extern "C" fn agent_middleware(perform: Perform, next: DynamicNext) -> Result<Effect> {
    info!("before");
    let v = (next.n)(perform);
    info!("after");
    v
}
