use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use crate::net::types::{FetchResult, NetError, NetResponseMeta};

// Simple waiter for coalescing responses: many listeners, one completion.
#[derive(Default)]
pub struct Waiter {
    listeners: Mutex<Vec<oneshot::Sender<FetchResult>>>,
}

impl Waiter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { listeners: Mutex::new(Vec::new()) })
    }

    pub async fn register(&self, tx: oneshot::Sender<FetchResult>) {
        self.listeners.lock().await.push(tx);
    }

    pub async fn finish(self: &Arc<Self>, result: FetchResult) {
        let mut ls = self.listeners.lock().await;
        for tx in ls.drain(..) {
            let _ = tx.send(result.clone_for_fanout());
        }
    }
}

impl FetchResult {
    pub fn clone_for_fanout(&self) -> FetchResult {
        match self {
            FetchResult::Buffered { meta, body } => FetchResult::Buffered {
                meta: NetResponseMeta {
                    final_url: meta.final_url.clone(),
                    status: meta.status,
                    reason: meta.reason.clone(),
                    headers: meta.headers.clone(),
                },
                body: body.clone(),
            },
            FetchResult::Stream { .. } => {
                // Cannot clone a streaming body; return an error instead.
                FetchResult::Error(super::types::NetError::Canceled)
            }
            FetchResult::Error(e) => FetchResult::Error(NetError::from(e.clone())),
        }
    }
}