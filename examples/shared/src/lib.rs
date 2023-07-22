use anyhow::Result;
use chrono::Duration;
use dispatcher::Dispatch;
use serde::Deserialize;
use tracing::*;

use plugins_agent_sys::{Agent, AgentBridge};
use plugins_core::{
    carrying::model::CarryingEvent,
    library::{
        model::Serialize,
        plugin::{get_my_session, Audience, Incoming, Surroundings, ToJson, When},
    },
    tools,
};
use plugins_dynlib::{recv, send, DynMessage, DynamicHost};

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
    fn have_surroundings(&mut self, surroundings: Surroundings) -> Result<()> {
        let (world, living, area) = surroundings.unpack();

        // info!("surroundings {:?}", surroundings);
        // let area = area.entity()?;
        // area.set_name("My world now!")?;

        info!("world {:?}", world);
        info!("living {:?}", living);
        info!("area {:?}", area);
        let area_of = tools::area_of(&living)?;
        info!("area-of: {:?}", area_of);

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

        get_my_session()?.schedule(
            "example-test",
            When::Interval(Duration::seconds(10)),
            &ExampleFuture::Wakeup,
        )?;

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

plugins_dynlib::export_plugin!(agent_initialize, agent_tick);

fn default_plugin_setup(dh: &dyn DynamicHost) {
    if !dispatcher::has_been_set() {
        let subscriber = dh.tracing_subscriber();
        let dispatch = Dispatch::new(subscriber);
        match dispatcher::set_global_default(dispatch) {
            Err(_) => println!("Error configuring plugin tracing"),
            Ok(_) => {}
        };
    }
}

#[allow(improper_ctypes_definitions)]
extern "C" fn agent_initialize(dh: &mut dyn DynamicHost) {
    default_plugin_setup(dh);
}

#[allow(improper_ctypes_definitions)]
extern "C" fn agent_tick(dh: &mut dyn DynamicHost) {
    let mut bridge = Box::new(AgentBridge::<ExampleAgent>::new(ExampleAgent::default()));
    let sending = match bridge.tick(|| match recv::<DynMessage>(dh) {
        Some(m) => match m {
            DynMessage::Payload(m) => Some(m),
            DynMessage::Query(_) => unimplemented!(),
        },
        None => None,
    }) {
        Ok(sending) => {
            dh.state(Box::into_raw(bridge) as *const std::ffi::c_void);
            sending
        }
        Err(e) => {
            error!("{:?}", e);
            vec![]
        }
    };

    for m in sending {
        send(dh, DynMessage::Query(m));
    }
}
