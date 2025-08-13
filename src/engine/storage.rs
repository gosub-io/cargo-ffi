use std::sync::Arc;

pub mod types;
pub mod area;
pub mod event;
pub mod service;

pub mod local {
    pub mod sqlite_store;
    pub mod in_memory;
}

pub mod session {
    pub mod in_memory;
}

#[derive(Clone)]
pub struct StorageHandles {
    pub local: Arc<dyn StorageArea>,
    pub session: Arc<dyn StorageArea>,
}

pub use types::PartitionKey;
pub use area::{StorageArea, LocalStore, SessionStore};
pub use service::{StorageService, Subscription};
pub use local::sqlite_store::SqliteLocalStore;
pub use session::in_memory::InMemorySessionStore;
pub use event::StorageEvent;

