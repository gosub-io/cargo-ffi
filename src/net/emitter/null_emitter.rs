use crate::net::events::{NetEvent, NetObserver};

pub struct NullEmitter {
}

impl NetObserver for NullEmitter {
    fn on_event(&self, _ev: NetEvent) {
        // Do nothing with the event
    }
}