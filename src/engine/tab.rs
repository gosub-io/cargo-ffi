//! Tab system: [`Tab`], [`Tick`](crate::engine::tick::TickResult), and [`TabId`].
//!
//! A **tab** is a single browsing context within a [`Zone`](crate::engine::zone::Zone):
//! it owns an [`EngineInstance`](crate::EngineInstance), a [`Viewport`], and state
//! for loading+rendering a page. Tabs share zone resources such as cookies and storage.
//!
//! # Lifecycle
//!
//! Tabs run a small state machine (`[`TabState`]`) driven by [`Tab::tick`]:
//!
//! 1. `Idle` → user action
//! 2. `PendingLoad(url)` → start network → `Loading`
//! 3. `Loading` → on success: `Loaded` (and set raw HTML) / on error: `Failed`
//! 4. `Loaded` → `PendingRendering(viewport)` → `Rendering` → `Rendered` → `Idle`
//!
//! The engine calls `tick()` regularly (e.g., each frame or via a scheduler).
//! `tick()` returns a [`TickResult`](crate::engine::tick::TickResult) indicating whether
//! the tab needs redraw and/or committed a new URL.
//!
//! # Example
//!
//! ```no_run
//! use gosub_engine::{GosubEngine, Viewport, EngineCommand};
//! use url::Url;
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone_builder().create().unwrap();
//!
//! // Create a tab
//! let tab_id = engine.open_tab(zone_id, &Viewport::new(0, 0, 800, 600)).unwrap();
//!
//! // Navigate
//! engine.execute_command(tab_id, EngineCommand::Navigate(Url::parse("https://example.com").unwrap())).unwrap();
//!
//! // Drive the engine
//! let results = engine.tick();
//! if let Some(res) = results.get(&tab_id) {
//!     if res.needs_redraw { /* schedule a repaint */ }
//! }
//! ```


use std::sync::Arc;
use std::time::Instant;
use gtk4::cairo;
use serde::__private::from_utf8_lossy;
use tokio::runtime::Runtime;
use url::Url;
use uuid::Uuid;
use crate::{EngineCommand, EngineEvent, EngineInstance};
use crate::engine::tick::TickResult;
use crate::viewport::Viewport;
use crate::engine::cookies::CookieJarHandle;
use crate::engine::storage::{PartitionKey, StorageEvent, StorageHandles};
use crate::engine::storage::types::PartitionPolicy;
use crate::engine::zone::ZoneId;

/// A unique identifier for a browser tab within a [`GosubEngine`](crate::engine::GosubEngine).
///
/// Internally, a `TabId` is a wrapper around a [`Uuid`], ensuring global
/// uniqueness for each tab opened in the engine. `TabId` implements
/// common traits such as `Copy`, `Clone`, `Eq`, `Hash`, and ordering traits,
/// so it can be freely duplicated, compared, sorted, or used as a key in
/// hash maps.
///
/// **Note:** The use of [`Uuid`] is an implementation detail and may change
/// in the future without notice. You should not depend on the internal
/// representation; always treat `TabId` as an opaque handle.
///
/// # Purpose
///
/// Tabs in Gosub are lightweight handles representing an open page
/// (or a rendering context) within a [`Zone`](crate::engine::zone::Zone). `TabId` allows the engine
/// and user code to unambiguously reference and operate on a specific tab,
/// even if tabs are opened or closed dynamically.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Current state of the tab. This is a state machine that defines what the tab is doing at the moment.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    /// Tab is idle (no pending network, animations, or rendering).
    #[default]
    Idle,

    /// A navigation has been requested but not started yet.
    /// The next `tick()` will transition to [`TabState::Loading`].
    PendingLoad(Url),

    /// The tab is fetching network resources (main document).
    /// When done, transitions to [`TabState::Loaded`] on success or [`TabState::Failed`] on error.
    Loading,

    /// Main document has been received and staged into the engine.
    /// The next `tick()` will begin rendering via [`TabState::PendingRendering`].
    Loaded,

    /// A render has been requested for the given viewport.
    PendingRendering(Viewport),

    /// The engine is producing a new surface for the current content.
    Rendering,

    /// A new surface is ready for painting. The next `tick()` typically
    /// returns to [`TabState::Idle`] and sets `needs_redraw = true` in [`TickResult`].
    Rendered,

    /// A fatal error occurred while loading or rendering.
    Failed(String),
}

/// Activity mode for a [`Tab`]. Schedulers can allocate CPU/time by mode.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TabMode {
    /// Foreground: fully active (network, layout, paint, animations ~60 Hz).
    Active,

    /// Background with animations alive but throttled (e.g., ~10 Hz).
    BackgroundLive,

    /// Background with minimal ticking (network/JS timers only, e.g., ~1 Hz).
    BackgroundIdle,

    /// Suspended: no ticking until an event or visibility change.
    Suspended,
}


/// A single browsing context inside a [`Zone`](crate::engine::zone::Zone).
///
/// A [`Tab`] owns an [`EngineInstance`](crate::EngineInstance) and tracks its
/// viewport, loading/rendering state, current/pending URL, favicon/title, and
/// per-tab storage partitioning. Tabs share the zone's cookie jar and storage.
///
/// Drive a tab by calling [`tick`](Tab::tick) regularly and by injecting
/// [`EngineEvent`](crate::EngineEvent) and [`EngineCommand`](crate::EngineCommand)
/// from your UI.
///
/// Typical loop: `execute_command(Navigate) → tick() → (Loaded) → tick() → (Rendered)`
/// and then paint the returned surface.
pub struct Tab {
    /// ID of the tab
    pub id: TabId,
    /// ID of the zone in which this tab resides
    pub zone_id: ZoneId,
    /// Engine instance running for this tab
    pub instance: EngineInstance,
    /// State of the tab (idle, loading, loaded, etc.)
    pub state: TabState,

    /// Current (or wanted) viewport for rendering
    pub viewport: Viewport,

    /// Current tab mode (idle, live, background)
    pub mode: TabMode,
    /// When was the last tick?
    pub last_tick: Instant,

    /// Favicon binary data for the current tab
    pub favicon: Vec<u8>,
    /// Title of the current tab
    pub title: String,

    /// URL that ready to load or is loading
    pub pending_url: Option<Url>,
    /// Current URL that is now loaded
    pub current_url: Option<Url>,
    /// Is the current URL being loaded
    pub is_loading: bool,
    /// Is there an error in the current tab?
    pub is_error: bool,

    /// Cookie jar for this tab. This is shared with the rest of the zone tabs
    pub cookie_jar: Option<CookieJarHandle>,

    /// Storage partition key
    pub partition_key: PartitionKey,
    /// Storage partition policy
    pub partition_policy: PartitionPolicy,
}

impl Tab {
    /// Create a new tab bound to `zone_id`, with a runtime, initial viewport,
    /// and an optional zone-shared cookie jar handle.
    ///
    /// The tab starts in [`TabState::Idle`], [`TabMode::Active`], and with
    /// [`PartitionKey::None`]/[`PartitionPolicy::TopLevelOrigin`].
    pub fn new(
        zone_id: ZoneId,
        runtime: Arc<Runtime>,
        viewport: &Viewport,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Self {
        Self {
            id: TabId::new(),
            zone_id,
            state: TabState::Idle,
            instance: EngineInstance::new(runtime),
            viewport: viewport.clone(),

            favicon: vec![],                    // Placeholder for favicon data
            title: "New Tab".to_string(),       // Title of the new tab

            pending_url: None,
            current_url: None,
            is_loading: false,
            is_error: false,

            mode: TabMode::Active,                  // Default mode is active
            last_tick: Instant::now(),

            cookie_jar,
            partition_key: PartitionKey::None,      // Start with no partition key
            partition_policy: PartitionPolicy::TopLevelOrigin,
        }
    }

    /// Navigate to a URL (string is parsed into a `Url`). On success, moves the
    /// tab to [`TabState::PendingLoad(url)`]. Invalid URLs are ignored and logged.
    pub fn navigate_to(&mut self, url: impl Into<String>) {
        let url = match Url::parse(&url.into()) {
            Ok(url) => url,
            Err(e) => {
                // Can't parse string to a URL to load
                eprintln!("Cannot parse URL: {}", e);
                return
            }
        };

        self.state = TabState::PendingLoad(url.into());
        self.is_loading = true;
    }

    /// Bind local+session storage handles into the underlying engine instance.
    /// Call this after creating the tab or when the zone’s storage changes.
    pub fn bind_storage(&mut self, storage: StorageHandles) {
        self.instance.bind_storage(storage.local, storage.session);
    }

    /// Set a new viewport and schedule a re-render
    /// by transitioning to [`TabState::PendingRendering`].
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        self.state = TabState::PendingRendering(self.viewport.clone())
    }

    /// Advance the tab’s state machine once and return a [`TickResult`]
    /// indicating whether a redraw is needed and whether a page was committed.
    ///
    /// **Returns**
    /// - `needs_redraw = true` when a new surface is ready to paint
    /// - `page_loaded = true` when a navigation commits
    pub fn tick(&mut self) -> TickResult {
        let mut result = TickResult::default();

        match self.state.clone() {
            TabState::Idle => {
                // Nothing to do
            }

            // Start loading the URL
            TabState::PendingLoad(url) => {
                self.state = TabState::Loading;
                self.is_loading = true;
                self.instance.start_loading(url.clone());
                self.pending_url = Some(url.clone());
            }

            // Poll the loading task until it's completed (or failed)
            TabState::Loading => {
                if let Some(done) = self.instance.poll_loading() {
                    match done {
                        Ok(resp) => {
                            println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

                            // Store cookies from the response in the cookie jar
                            if let Some(cookie_jar) = &self.cookie_jar {
                                cookie_jar.write().unwrap().store_response_cookies(
                                    &resp.url,
                                    &resp.headers,
                                );
                            }

                            // Set tab state
                            self.state = TabState::Loaded;
                            self.is_loading = false;
                            self.instance.set_raw_html(from_utf8_lossy(resp.body.as_slice()).to_string());
                            self.pending_url = None;
                            self.current_url = Some(resp.url.clone());

                            // Set result
                            result.page_loaded = true;
                            result.commited_url = Some(resp.url.clone());
                        }
                        Err(e) => {
                            println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

                            self.state = TabState::Failed(e);
                            self.is_loading = false;
                            self.is_error = true;
                            result.needs_redraw = true;
                        }
                    }
                }
            }

            // Start rendering after we finished loading
            TabState::Loaded => {
                println!("Tabstate loaded, starting rendering");
                self.state = TabState::PendingRendering(self.viewport.clone());
            }

            TabState::PendingRendering(viewport) => {
                self.instance.start_rendering(viewport);
                self.state = TabState::Rendering;
            }

            // Notify the outside world that we have something to paint, and we can go back to idle state.
            TabState::Rendered => {
                self.state = TabState::Idle;
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

                result.needs_redraw = true;
            }

            TabState::Failed(msg) => {
                self.instance.render_error(&msg, self.viewport.clone());
                self.state = TabState::Rendered;
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

                result.needs_redraw = true;
            }

            // Normally, rendering will take a while (async). Currently, it doesn't so we move directly
            // to a Rendered state.
            TabState::Rendering => {
                self.state = TabState::Rendered;
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

            }
        }

        result
    }


    /// Handle an external UI event (scroll, mouse, keyboard, resize).
    /// Typically forwarded from your toolkit.
    pub(crate) fn handle_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Scroll { dx, dy } => {
                println!("Scrolling tab {:?} by dx: {}, dy: {}", self.id, dx, dy);

                self.set_viewport(Viewport::new(
                    self.viewport.x + dx as i32,
                    self.viewport.y + dy as i32,
                    self.viewport.width,
                    self.viewport.height
                ))
            }
            EngineEvent::MouseMove { x, y } => {
                println!("Mouse moved on tab {:?} to position ({}, {})", self.id, x, y);
            }
            EngineEvent::MouseDown { button, x, y } => {
                println!("Mouse down event on tab {:?} at position ({}, {}) with button {:?}", self.id, x, y, button);
            }
            EngineEvent::MouseUp { button, x, y } => {
                println!("Mouse up event on tab {:?} at position ({}, {}) with button {:?}", self.id, x, y, button);
            }
            EngineEvent::KeyDown { key } => {
                println!("Key down event on tab {:?} for key: {}", self.id, key);
            }
            EngineEvent::KeyUp { key } => {
                println!("Key up event on tab {:?} for key: {}", self.id, key);
            }
            EngineEvent::InputChar { character } => {
                println!("Input character event on tab {:?}: '{}'", self.id, character);
            }
            EngineEvent::Resize { width, height } => {
                println!("Resize event on tab {:?}: new size {}x{}", self.id, width, height);
                self.set_viewport(Viewport::new(self.viewport.x, self.viewport.y, width, height))
            }
        }
    }

    /// Execute a high-level engine command (navigate, reload).
    pub fn execute_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Navigate(url) => {
                println!("Loading URL '{}' in tab {:?}", url, self.id);
                // self.pending_url = Some(url.clone());f
                self.state = TabState::PendingLoad(url);
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());
            }
            EngineCommand::Reload() => {
                let Some(url) = self.current_url.clone() else {
                    return;
                };

                println!("Reloading URL '{}' in tab {:?}", url, self.id);
                // self.pending_url = Some(url.clone());f
                self.state = TabState::PendingLoad(url);
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());
            }
        }
    }

    /// Borrow the current rendered surface, if any.
    /// Paint this inside your toolkit’s draw callback.
    pub fn get_surface(&self) -> Option<&cairo::ImageSurface> {
        self.instance.surface()
    }


    /// Dispatch a storage event to same-origin documents in this tab (placeholder).
    /// Intended for HTML5 storage event semantics.
    pub fn dispatch_storage_event_to_same_origin_docs(
        &mut self,
        _origin: &url::Origin,
        _include_iframes: bool,
        _ev: &StorageEvent,
    ) {
        // Pseudocode stuff.. need to fill in what it actually needs to do
        // for doc in self.iter_documents(include_iframes) {
        //     if doc.origin() == origin {
        //         // Don’t fire the event at the *mutating document* itself.
        //         if Some(self.id) == ev.source_tab && doc.is_the_mutating_document() {
        //             continue;
        //         }
        //         doc.runtime().dispatch_storage_event(
        //             ev.key.as_deref(),
        //             ev.old_value.as_deref(),
        //             ev.new_value.as_deref(),
        //             doc.url().to_string(),
        //             match ev.scope { StorageScope::Local => "local", StorageScope::Session => "session" }
        //         );
        //     }
        // }
    }
}