//! High-level document loading utilities.

use bytes::Bytes;
use http::HeaderMap;
use url::Url;
use crate::net::mime::MimeKind;
use serde_json::Value as JsonValue;

/// A parsed document, represented as a string for now
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document(pub String);

/// The resource that was loaded
#[derive(Debug)]
pub enum Resource {
    /// HTML document
    Html(Document),
    /// JSON document
    Json(JsonValue),
    /// (raw) image
    Image { bytes: Bytes, mime: String }, // raw bytes (decode lazily in UI)
    /// Text content
    Text { text: String, mime: String },  // e.g. text/plain, css, etc.
    /// Binary content
    Binary { bytes: Bytes, mime: String } // downloadable/hex viewer
}

/// Metadata about a loaded resource
#[derive(Debug)]
pub struct ResourceMeta {
    /// Final URL that is loaded
    pub final_url: Url,
    /// MimeKind detected
    pub mime: MimeKind,
    /// Charset detected
    pub charset: Option<String>,
    /// Content length (if provided)
    pub content_length: Option<u64>,
    /// E-Tag
    pub etag: Option<String>,
    /// Last modifier
    pub last_modified: Option<String>,
    /// Headers
    pub headers: HeaderMap,
}

/// The result of a (successful) navigation load
#[derive(Debug)]
pub struct NavigationOutput {
    pub meta: ResourceMeta,
    pub resource: Resource,
}

/// Result type for navigation operations
pub type NavResult<T> = Result<T, NavigationError>;
pub type ResourceLoadResult = NavResult<NavigationOutput>;

#[derive(thiserror::Error, Debug)]
pub enum NavigationError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("network error: {0}")]
    NetworkError(String),

    #[error("io cancelled: {0}")]
    Cancelled(String),

    #[error("io timeout: {0}")]
    Timeout(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
//
// /// Load the main document for a top-level navigation.
// pub async fn load_main_document(
//     tab_id: crate::tab::TabId,
//     nav_id: NavigationId,
//     url: Url,
//     cancel: CancellationToken,
//     ignore_cache: bool, // reserved
//     event_tx: broadcast::Sender<EngineEvent>,
//     kind: ResourceKind,
//     initiator: Initiator,
// ) -> ResourceLoadResult {
//     // Early cancellation check
//     if cancel.is_cancelled() {
//         return Err(NavigationError::Cancelled("navigation cancelled before start".into()));
//     }
//
//     let req_id = RequestId::new();
//     let emitter = EngineEventEmitter { tab_id, nav_id, req_id, event_tx, kind, initiator };
//
//     if !ignore_cache {
//         // We need to make sure we check the cache first before requesting data
//     }
//
//     fetch_resource(nav_id, url, cancel, Some(&emitter)).await
// }
