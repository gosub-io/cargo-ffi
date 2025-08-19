//! Zone configuration module.
//!
//! This module defines the [`ZoneConfig`] struct, which controls
//! properties and limits for a single [`Zone`](crate::engine::Zone).
//!
//! A `Zone` acts as an isolation boundary in the Gosub engine, similar
//! to a browser profile or container. The configuration determines how
//! that zone behaves, including limits on resource usage such as how
//! many tabs may be opened.
//!
//! # Current fields
//!
//! - [`max_tabs`]: Maximum number of tabs that may be opened in the zone.
//!
//! # Example
//!
//! ```rust
//! use gosub_engine::zone::ZoneConfig;
//!
//! // Create a zone configuration that allows up to 10 tabs.
//! let config = ZoneConfig { max_tabs: 10 };
//! ```
//!
//! In the future, additional fields may be added to `ZoneConfig` to
//! support features such as storage options, cookie policies, or
//! encryption keys.

/// Zone configuration for the Gosub engine.
#[derive(Debug, Clone)]
pub struct ZoneConfig {
    /// How many tabs might be opened in the zone
    pub max_tabs: usize,
}