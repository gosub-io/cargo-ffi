//! Network utilities for making HTTP requests.
//!
mod fetch;
mod response;
mod loader;
pub mod types;
mod events;
mod emitter;

pub use loader::load_main_document;
pub use loader::DocumentLoadResult;
pub use fetch::fetch;
pub use response::Response;
