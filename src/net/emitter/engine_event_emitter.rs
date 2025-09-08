use tokio::sync::broadcast;
use crate::engine::events::{CancelReason, ResourceEvent, PRIO_DEFAULT};
use crate::engine::types::{NavigationId, RequestId};
use crate::events::EngineEvent;
use crate::tab::TabId;
use crate::net::events::{NetEvent, NetObserver};
use crate::net::types::{Initiator, ResourceKind};

pub struct EngineEventEmitter {
    pub tab_id: TabId,
    pub nav_id: NavigationId,
    pub req_id: RequestId,
    pub event_tx: broadcast::Sender<EngineEvent>,
    pub kind: ResourceKind,
    pub initiator: Initiator,
}

impl EngineEventEmitter {
    fn emit(&self, ev: ResourceEvent) {
        let _ = self.event_tx.send(EngineEvent::Resource {
            tab_id: self.tab_id,
            event: ev,
        });
    }
}

impl NetObserver for EngineEventEmitter {
    fn on_event(&self, ev: NetEvent) {
        match ev {
            NetEvent::Started { url } => {
                self.emit(ResourceEvent::Started {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    kind: self.kind,
                    initiator: self.initiator,
                    priority: PRIO_DEFAULT,
                });
            }
            NetEvent::Redirected { from, to, status } => {
                self.emit(ResourceEvent::Redirected {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    from: from.to_string(),
                    to: to.to_string(),
                    status,
                });
            }
            NetEvent::ResponseHeaders { .. } => {
                // self.emit(ResourceEvent::Progress {
                //     nav_id,
                //     req_id,
                //     received_bytes: 0,
                // });
            }
            NetEvent::Progress { received_bytes } => {
                self.emit(ResourceEvent::Progress {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    received_bytes,
                });
            }
            NetEvent::Finished { url, bytes, elapsed, content_type } => {
                self.emit(ResourceEvent::Finished {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    bytes,
                    content_type,
                    elapsed: Some(elapsed),
                });
            }
            NetEvent::Failed { url, error } => {
                self.emit(ResourceEvent::Failed {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    error,
                });
            }
            NetEvent::Cancelled { url, reason } => {
                self.emit(ResourceEvent::Cancelled {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    reason: CancelReason::Custom(reason.to_string()),
                });
            }
        }
    }
}