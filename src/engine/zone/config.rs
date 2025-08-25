//! Zone configuration.
//!
//! `ZoneConfig` controls properties and limits for a single
//! [`Zone`](crate::zone::Zone) in the Gosub engine. A *zone* acts like a
//! browser profile/container: it defines behavior (e.g. JS/images enabled),
//! identity (e.g. user agent, languages), and limits (e.g. max tabs).
//!
//! `ZoneConfig` provides sensible defaults via [`Default`] and a fluent
//! [`ZoneConfig::builder()`] for customization with validation.
//!
//! # Examples
//!
//! ## Use defaults
//! ```rust
//! use gosub_engine::zone::ZoneConfig;
//! let cfg = ZoneConfig::default();
//! assert_eq!(cfg.max_tabs, 16);
//! ```
//!
//! ## Customize with the builder
//! ```rust
//! use gosub_engine::zone::ZoneConfig;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let cfg = ZoneConfig::builder()
//!     .max_tabs(10)
//!     .user_agent("Gosub/0.1")
//!     .accept_languages("en-US,en;q=0.9,nl;q=0.8")
//!     .javascript_enabled(true)
//!     .images_enabled(true)
//!     .font_scale(1.25)
//!     .minimum_font_size(12)
//!     .build()?; // returns Result<ZoneConfig, ZoneConfigError>
//! # Ok(()) }
//! ```
//!
//! # Fields (summary)
//! - `max_tabs`: Maximum number of tabs allowed in the zone (default: 16).
//! - `user_agent`: Optional UA string to send with requests.
//! - `accept_languages`: Optional `Accept-Language` header value.
//! - `do_not_track`: Send `DNT: 1` header if `true`.
//! - `javascript_enabled`: Execute JavaScript if `true`.
//! - `images_enabled`: Load images if `true`.
//! - `plugins_enabled`: Enable plugins if `true`.
//! - `font_scale`: UI/content scale factor (validated range `0.25..=10.0`).
//! - `default_font_family`: Optional default font family name.
//! - `default_font_size`: Default font size in CSS px (default: 16).
//! - `minimum_font_size`: Minimum allowed font size in CSS px (must be ≤ `default_font_size`).
//! - `enable_local_file_access`: Allow `file://` (sandboxing concerns).
//!
//! # Notes
//!
//! Note that most of these fields are not implemented but are here to show
//! the intended design. The actual implementation may change without notice.
//!
//! # Errors
//!
//! Builder validation can return [`ZoneConfigError`] if values are invalid
//! (e.g. `font_scale` outside `0.25..=10.0`, `minimum_font_size > default_font_size`,
//! or `max_tabs == 0`).

use std::fmt;

#[derive(Debug, Clone)]
pub struct ZoneConfig {
    pub max_tabs: usize,
    pub user_agent: Option<String>,
    pub accept_languages: Option<String>,
    pub do_not_track: bool,
    pub javascript_enabled: bool,
    pub images_enabled: bool,
    pub plugins_enabled: bool,
    pub font_scale: f32,
    pub default_font_family: Option<String>,
    pub default_font_size: u32,
    pub minimum_font_size: u32,
    pub enable_local_file_access: bool,
}

impl Default for ZoneConfig {
    fn default() -> Self {
        Self {
            max_tabs: 16,
            user_agent: None,
            accept_languages: None,
            do_not_track: false,
            javascript_enabled: true,
            images_enabled: true,
            plugins_enabled: false,
            font_scale: 1.0,
            default_font_family: None,
            default_font_size: 16,
            minimum_font_size: 0,
            enable_local_file_access: false,
        }
    }
}

impl ZoneConfig {
    pub fn builder() -> ZoneConfigBuilder {
        ZoneConfigBuilder::default()
    }
}

/// Builder for [`ZoneConfig`], mirroring `EngineConfigBuilder`.
#[derive(Debug, Clone)]
pub struct ZoneConfigBuilder {
    inner: ZoneConfig,
}

impl Default for ZoneConfigBuilder {
    fn default() -> Self {
        Self { inner: ZoneConfig::default() }
    }
}

impl ZoneConfigBuilder {
    #[inline]
    fn map(mut self, f: impl FnOnce(&mut ZoneConfig)) -> Self {
        f(&mut self.inner);
        self
    }

    pub fn max_tabs(self, n: usize) -> Self { self.map(|c| c.max_tabs = n) }
    pub fn user_agent<S: Into<String>>(self, ua: S) -> Self { self.map(|c| c.user_agent = Some(ua.into())) }
    pub fn accept_languages<S: Into<String>>(self, langs: S) -> Self { self.map(|c| c.accept_languages = Some(langs.into())) }
    pub fn do_not_track(self, dnt: bool) -> Self { self.map(|c| c.do_not_track = dnt) }
    pub fn javascript_enabled(self, on: bool) -> Self { self.map(|c| c.javascript_enabled = on) }
    pub fn images_enabled(self, on: bool) -> Self { self.map(|c| c.images_enabled = on) }
    pub fn plugins_enabled(self, on: bool) -> Self { self.map(|c| c.plugins_enabled = on) }
    pub fn font_scale(self, scale: f32) -> Self { self.map(|c| c.font_scale = scale) }
    pub fn default_font_family<S: Into<String>>(self, fam: S) -> Self { self.map(|c| c.default_font_family = Some(fam.into())) }
    pub fn default_font_size(self, px: u32) -> Self { self.map(|c| c.default_font_size = px) }
    pub fn minimum_font_size(self, px: u32) -> Self { self.map(|c| c.minimum_font_size = px) }
    pub fn enable_local_file_access(self, on: bool) -> Self { self.map(|c| c.enable_local_file_access = on) }

    /// Apply multiple changes in one go.
    pub fn with(self, f: impl FnOnce(&mut ZoneConfig)) -> Self { self.map(f) }

    /// Validate and build the final config.
    pub fn build(self) -> Result<ZoneConfig, ZoneConfigError> {
        validate(&self.inner)?;
        Ok(self.inner)
    }
}

// ---------- Validation ----------

#[derive(Debug, Clone)]
pub enum ZoneConfigError {
    InvalidFontScale(f32),
    MinFontLarger { min: u32, default: u32 },
    ZeroTabs,
}

impl fmt::Display for ZoneConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZoneConfigError::InvalidFontScale(s) =>
                write!(f, "font_scale {s} is out of range (expected 0.25..=10.0)"),
            ZoneConfigError::MinFontLarger { min, default } =>
                write!(f, "minimum_font_size ({min}) > default_font_size ({default})"),
            ZoneConfigError::ZeroTabs =>
                write!(f, "max_tabs must be at least 1"),
        }
    }
}
impl std::error::Error for ZoneConfigError {}

fn validate(c: &ZoneConfig) -> Result<(), ZoneConfigError> {
    if !(0.25..=10.0).contains(&c.font_scale) {
        return Err(ZoneConfigError::InvalidFontScale(c.font_scale));
    }
    if c.minimum_font_size > c.default_font_size {
        return Err(ZoneConfigError::MinFontLarger {
            min: c.minimum_font_size,
            default: c.default_font_size,
        });
    }
    if c.max_tabs == 0 {
        return Err(ZoneConfigError::ZeroTabs);
    }
    Ok(())
}