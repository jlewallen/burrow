use std::rc::Rc;

use anyhow::Result;
use chrono::Duration;
use kernel::Perform;
use tracing::*;

use dynlib_sys::{prelude::*, DynamicNext};
use plugins_core::{
    carrying::model::CarryingEvent,
    library::{
        model::{Deserialize, Serialize},
        plugin::{
            get_my_session, Audience, Effect, Evaluator, Incoming, Reply, Surroundings, ToJson,
            When,
        },
    },
    tools,
};

#[derive(Debug, Serialize, Deserialize)]
enum ExampleFuture {
    Wakeup,
}

impl TryInto<ExampleFuture> for Incoming {
    type Error = anyhow::Error;

    fn try_into(self) -> std::result::Result<ExampleFuture, Self::Error> {
        Ok(serde_json::from_slice(&self.serialized)?)
    }
}

impl ToJson for ExampleFuture {
    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
        Ok(serde_json::to_value(self)?)
    }
}

#[derive(Default)]
struct ExampleAgent {}

impl Agent for ExampleAgent {
    fn initialize(&mut self) -> Result<()> {
        get_my_session()?.schedule(
            "example-test",
            When::Interval(Duration::seconds(10)),
            &ExampleFuture::Wakeup,
        )?;

        Ok(())
    }

    fn have_surroundings(&mut self, surroundings: Surroundings) -> Result<()> {
        let (world, living, area) = surroundings.unpack();

        trace!("world {:?}", world);
        trace!("living {:?}", living);
        trace!("area {:?}", area);
        let area_of = tools::area_of(&living)?;
        trace!("area-of: {:?}", area_of);

        if false {
            for dropping in tools::contained_by(&area)? {
                let raise = CarryingEvent::ItemDropped {
                    living: living.clone(),
                    item: dropping,
                    area: area.clone(),
                };
                get_my_session()?.raise(Audience::Area(area.key().clone()), Box::new(raise))?;
            }
        }

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

impl Evaluator for ExampleAgent {
    fn evaluate(
        &self,
        _perform: &dyn plugins_core::library::plugin::Performer,
        consider: plugins_core::library::plugin::Evaluable,
    ) -> Result<Vec<Effect>> {
        info!("try-parse {:?}", consider);

        Ok(vec![Effect::Reply(Rc::new(ExampleReply {}))])
    }
}

#[derive(Debug, Serialize)]
struct ExampleReply {}

impl ToJson for ExampleReply {
    fn to_json(&self) -> std::result::Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

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
    info!("after = {:?}", v);
    v
}
