use anyhow::Result;
use bincode::{Decode, Encode};
use chrono::Duration;
use dispatcher::Dispatch;
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
use plugins_dynlib::{DynMessage, DynamicHost};

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

#[derive(Debug, Serialize)]
enum ExampleFuture {
    Wakeup,
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

    fn deliver(&mut self, _incoming: Incoming) -> Result<()> {
        Ok(())
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

fn recv<T: Decode>(dh: &mut dyn DynamicHost) -> Option<T> {
    // For now this seems ok, but 'now' is basically the first test. So if
    // you end up here in the future I think you'll probably be better off
    // batching the protocol than you'll be worrying about memory
    // management.
    let mut buffer = [0; 65536];
    let len = dh.recv(&mut buffer);

    if len == 0 {
        return None;
    }

    if len > buffer.len() {
        error!(
            "Serialized message is larger than buffer (by {} bytes)",
            len - buffer.len()
        );
        return None;
    }

    match bincode::decode_from_slice(&buffer[..len], bincode::config::legacy()) {
        Ok((message, _)) => Some(message),
        Err(err) => {
            error!("Failed to deserialize message from host: {}", err);
            None
        }
    }
}

fn send<T: Encode>(dh: &mut dyn DynamicHost, message: T) {
    let encoded: Vec<u8> = match bincode::encode_to_vec(&message, bincode::config::legacy()) {
        Ok(encoded) => encoded,
        Err(err) => {
            error!("Failed to serialize event: {}", err);
            return;
        }
    };

    dh.send(&encoded);

    std::mem::drop(encoded);
}
