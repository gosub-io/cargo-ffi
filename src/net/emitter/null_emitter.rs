use crate::net::events::{NetEvent, NetObserver};

/// Emitter that will drop any events received
#[allow(unused)]
pub struct NullEmitter;

impl NetObserver for NullEmitter {
    fn on_event(&self, _ev: NetEvent) {
        // Do nothing with the event
    }
}