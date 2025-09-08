use bytes::Bytes;
use futures::Stream;
use std::{fmt, pin::Pin};
use std::fmt::Debug;
use std::sync::Arc;
use url::Url;
use crate::tab::TabId;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum Priority {
    High,
    Normal,
    Low,
    Idle,
}

/// Defines the different resource types that are available for loading
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ResourceKind {
    Document,
    Stylesheet,
    Script { blocking: bool },
    Image,
    Font,
    Media,
    Xhr,
    Fetch,
    WebSocket,
    Other,
}

pub type BodyStream = Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>;

// impl Debug for BodyStream {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         write!(f, "BodyStream")
//     }
// }

#[derive(Debug, Clone)]
pub struct FetchKey {
    pub url: Url,
    // Add Vary-relevant bits if desired:
    pub accept: Option<String>,
    pub range: Option<(u64, u64)>,
}

impl fmt::Display for FetchKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}


#[derive(Debug)]
pub struct FetchRequest {
    /// Key identifying the resource to fetch
    pub key: FetchKey,
    /// Priority of this request
    pub priority: Priority,
    /// Who initiated this request
    pub initiator: Initiator,
    /// What kind of resource is being fetched
    pub kind: ResourceKind,
    // Which Tab is asking for this resource
    pub tab_id: TabId,
    // whether to stream or buffer
    pub streaming: bool,
    // one shot reply
    pub reply: Option<tokio::sync::oneshot::Sender<FetchResult>>,
}

#[derive(Debug, Clone)]
pub struct NetResponseMeta {
    // Final URL after redirects
    pub final_url: Url,
    /// HTTP status code
    pub status: u16,
    /// HTTP status reason phrase
    pub reason: String,
    /// Response headers
    pub headers: reqwest::header::HeaderMap,
}

pub enum FetchResult {
    /// Streamed response body
    Stream { meta: NetResponseMeta, body: BodyStream },
    /// Buffered response body
    Buffered { meta: NetResponseMeta, body: Bytes },
    /// Errror
    Error(NetError),
}

impl Debug for FetchResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchResult::Stream { meta, .. } => f
                .debug_struct("FetchResult::Stream")
                .field("meta", meta)
                .finish(),
            FetchResult::Buffered { meta, body } => f
                .debug_struct("FetchResult::Buffered")
                .field("meta", meta)
                .field("body_len", &body.len())
                .finish(),
            FetchResult::Error(e) => f.debug_tuple("FetchResult::Error").field(e).finish(),
        }
    }
}

#[derive(Debug, thiserror::Error, Clone)]
pub enum NetError {
    #[error("network error: {0}")]
    Reqwest(Arc<reqwest::Error>),
    #[error("canceled")]
    Canceled,
}

impl From<reqwest::Error> for NetError {
    fn from(e: reqwest::Error) -> Self {
        NetError::Reqwest(Arc::new(e))
    }
}

/// Defines who initiated the resource load
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Initiator {
    /// Initiated by the user, UI, or link click
    Navigation,
    /// HTML Parser resource
    Parser,
    /// Initiated by a JS script (or Lua script) (fetch, XHR)
    Script,
    /// CSS @import, font-face
    CSS,
    /// Other undefined type of initiator
    Other,
}