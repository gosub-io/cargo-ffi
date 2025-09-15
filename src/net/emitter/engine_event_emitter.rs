use tokio::sync::broadcast;
use crate::engine::events::{CancelReason, ResourceEvent};
use crate::engine::types::{NavigationId, RequestId};
use crate::events::EngineEvent;
use crate::tab::TabId;
use crate::net::events::{NetEvent, NetObserver};
use crate::net::types::{Initiator, ResourceKind};

/// Converts NetEvents into EngineEvents and send them over to the event_tx channel
#[allow(unused)]
pub struct EngineEventEmitter {
    pub tab_id: TabId,
    pub nav_id: NavigationId,
    pub req_id: RequestId,
    pub event_tx: broadcast::Sender<EngineEvent>,
    pub kind: ResourceKind,
    pub initiator: Initiator,
}

impl EngineEventEmitter {
    #[allow(unused)]
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
            NetEvent::ResponseHeaders { url, status, headers } => {
                self.emit(ResourceEvent::Headers {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    status,
                    content_length: headers
                        .get(reqwest::header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok()),
                    content_type: headers
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .map(|s| s.to_string()),
                    headers: headers
                        .iter()
                        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
                        .collect(),
                });
            }
            NetEvent::Progress { received_bytes, expected_length, elapsed } => {
                self.emit(ResourceEvent::Progress {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    received_bytes,
                    expected_length,
                    elapsed,
                });
            }
            NetEvent::Finished { url, received_bytes, elapsed } => {
                self.emit(ResourceEvent::Finished {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url,
                    received_bytes,
                    elapsed: Some(elapsed),
                });
            }
            NetEvent::Failed { url, error } => {
                self.emit(ResourceEvent::Failed {
                    nav_id: self.nav_id,
                    req_id: self.req_id,
                    url: url.to_string(),
                    error: error.into(),
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