//! Network utilities for making HTTP requests.
//!
mod response;
pub mod loader;
pub mod types;
pub mod events;
mod emitter;
mod utils;
mod fetcher;
mod io_runtime;
mod fetch;
mod shared_body;
mod decision_hub;
mod pump;
mod fs_utils;
mod render_html;
mod decision;
mod router;

pub use decision_hub::DecisionToken;
pub use decision::decide_handling;
pub use decision::types::{DecisionOutcome, HandlingDecision, RenderTarget, RequestDestination};

pub use shared_body::SharedBody;

pub use io_runtime::IoHandle;
pub use io_runtime::spawn_io_thread;

pub use fetcher::FetcherConfig;

pub use utils::stream_to_bytes;

pub use router::route_response_for;
pub use router::RoutedOutcome;