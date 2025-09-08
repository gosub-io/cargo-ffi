use std::time::Duration;
use url::Url;

pub trait NetObserver: Send + Sync {
    fn on_event(&self, ev: NetEvent);
}

#[derive(Debug)]
pub enum NetEvent {
    Started {
        url: Url,
    },
    Redirected {
        from: Url,
        to: Url,
        status: u16,
    },
    ResponseHeaders {
        url: Url,
        status: u16,
        content_length: Option<u64>,
        content_type: Option<String>,
    },
    Progress {
        received_bytes: u64, // cumulative
    },
    Finished {
        url: Url,
        bytes: u64,
        elapsed: Duration,
        content_type: Option<String>,
    },
    Failed {
        url: Url,
        error: String,
    },
    Cancelled {
        url: Url,
        reason: &'static str,
    },
}