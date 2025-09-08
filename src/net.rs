//! Network utilities for making HTTP requests.
//!
mod fetch;
mod response;
mod loader;
pub mod types;
mod events;
mod emitter;
mod utils;
mod fetcher;
mod io_runtime;

pub use loader::load_main_document;
pub use loader::DocumentLoadResult;
pub use fetch::fetch;
pub use response::Response;

pub use io_runtime::IoHandle;
pub use fetcher::FetcherConfig;
pub use io_runtime::spawn_io_thread;

