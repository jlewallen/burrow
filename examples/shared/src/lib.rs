use bincode::{Decode, Encode};
use dispatcher::Dispatch;
use plugins_dynlib::{DynMessage, DynamicHost};
use plugins_rpc_proto::prelude::*;
use tracing::{dispatcher, error, info};

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
    while let Some(message) = recv::<DynMessage>(dh) {
        info!("message: {:?}", message);
    }

    send(dh, DynMessage::Query(Query::Complete));
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
