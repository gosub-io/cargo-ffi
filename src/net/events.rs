use std::time::Duration;
use http::HeaderMap;
use url::Url;
use crate::net::decider::DecisionToken;

/// A NetObserver allows to send NetEvents to emitters
pub trait NetObserver: Send + Sync {
    fn on_event(&self, ev: NetEvent);
}

/// Events that are send by the net::fetch() functions
#[derive(Debug)]
pub enum NetEvent {
    /// Io error happened
    Io {
        message: String
    },
    /// Warning happened
    Warning {
        url: Url,
        message: String
    },
    /// Resource is started to load
    Started {
        url: Url,
    },
    /// Resource is redirected to another URL
    Redirected {
        from: Url,
        to: Url,
        status: u16,
    },
    /// Response headers are received
    ResponseHeaders {
        url: Url,
        status: u16,
        headers: HeaderMap,
    },
    /// Progress updates, how many bytes already read
    Progress {
        // How many bytes received in this resource
        received_bytes: u64,
        // Expected length of the resource (if known)
        expected_length: Option<u64>,
        // Time spent loading so far on this resource
        elapsed: Duration,
    },
    /// Resource is finished
    Finished {
        received_bytes: u64,
        elapsed: Duration,
        url: Url,
    },
    /// Resource failed to fetch
    Failed {
        url: Url,
        error: anyhow::Error,
    },
    /// Resource fetching was cancelled
    Cancelled {
        url: Url,
        reason: &'static str,
    },
    /// Resource top has been loaded, and UA needs to decide what to do next
    DecisionRequired {
        url: Url,
        status: u16,
        headers: HeaderMap,
        content_length: Option<u64>,
        peek: Vec<u8>,
        token: DecisionToken
    },
}