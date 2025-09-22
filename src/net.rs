//! Network utilities for making HTTP requests.
//!
mod decision;
mod decision_hub;
mod emitter;
pub mod events;
mod fetch;
mod fetcher;
mod fs_utils;
mod io_runtime;
mod pump;
mod router;
mod shared_body;
pub mod types;
mod utils;
pub mod req_ref_tracker;

pub use decision::decide_handling;
pub use decision::types::{DecisionOutcome, HandlingDecision, RenderTarget, RequestDestination};
pub use decision_hub::DecisionToken;

pub use shared_body::SharedBody;

pub use io_runtime::spawn_io_thread;
pub use io_runtime::submit_to_io;
pub use io_runtime::IoHandle;

pub use fetcher::FetcherConfig;

pub use utils::stream_to_bytes;

pub use router::route_response_for;
pub use router::RoutedOutcome;

pub use fetcher::FetchInflightMap;
