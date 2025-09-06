mod handle;
mod options;
pub mod services;
mod structs;
mod tab;
pub(crate) mod worker;

pub use handle::TabHandle;
pub use tab::*;

pub use options::TabCacheMode;
pub use options::TabCookieJar;
pub use options::TabDefaults;
pub use options::TabOverrides;
pub use options::TabStorageScope;

pub use structs::EffectiveTabServices;
pub use structs::TabSpawnArgs;

// Tab management and tab-related types.
//
// This module re-exports the main types and services for working with tabs in the engine.
// It includes tab handles, options, services, and internal structures for tab management.
