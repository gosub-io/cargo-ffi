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
pub mod mime;
mod decider;
mod pump;
mod fs_utils;
mod render_html;
mod decision;

pub use decider::DecisionToken;
pub use decision::decide_handling;
pub use decision::types::{DecisionOutcome, HandlingDecision, RenderTarget, RequestDestination};

pub use loader::ResourceLoadResult;
pub use loader::NavigationError;
pub use loader::Resource;

pub use shared_body::SharedBody;

pub use io_runtime::IoHandle;
pub use io_runtime::spawn_io_thread;

pub use fetcher::FetcherConfig;