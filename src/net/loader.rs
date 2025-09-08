//! High-level document loading utilities.
//!
//! Currently just wraps `net::fetch()` with cancellation support
//! and a future hook point for cache/redirect/CSP/etc.

use anyhow::Context;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::engine::types::{NavigationId, RequestId};
use crate::events::EngineEvent;
use crate::net::emitter::engine_event_emitter::EngineEventEmitter;
use crate::net::{fetch, Response};
use crate::net::types::{Initiator, ResourceKind};

pub type DocumentLoadResult = Result<Response, anyhow::Error>;

/// Load the main document for a top-level navigation.
///
/// - Honors `cancel` (returns `Err` if cancelled before completion)
/// - `ignore_cache` is a future hook; currently unused here
pub async fn load_main_document(
    tab_id: crate::tab::TabId,
    nav_id: NavigationId,
    url: Url,
    cancel: CancellationToken,
    ignore_cache: bool, // reserved
    event_tx: broadcast::Sender<EngineEvent>,
    kind: ResourceKind,
    initiator: Initiator,
) -> DocumentLoadResult {
    // Early cancellation check
    if cancel.is_cancelled() {
        anyhow::bail!("navigation cancelled before start");
    }

    let req_id = RequestId::new();

    let emitter = EngineEventEmitter {
        tab_id,
        nav_id,
        req_id,
        event_tx,
        kind,
        initiator,
    };

    if !ignore_cache {
        // We need to make sure we check the cache first before requesting data
    }

    let resp = fetch(url, cancel, Some(&emitter))
        .await
        .context("network fetch failed");

    Ok(resp?)
}
