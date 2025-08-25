//! Tab system: [`Tab`], [`Tick`](crate::engine::tick::TickResult), and [`TabId`].
//!
//! A **tab** is a single browsing context within a [`Zone`](crate::engine::zone::Zone):
//! it owns an [`BrowsingContext`](crate::BrowsingContext), a [`Viewport`], and state
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
//! use gosub_engine::{GosubEngine, EngineCommand};
//! use url::Url;
//!
//! let mut engine = GosubEngine::new(None);
//! let zone_id = engine.zone_builder().create().unwrap();
//!
//! // Create a tab
//! let tab_id = engine.open_tab_in_zone(zone_id).unwrap();
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

use crate::engine::cookies::CookieJarHandle;
use crate::engine::storage::types::PartitionPolicy;
use crate::engine::storage::{PartitionKey, StorageEvent, StorageHandles};
use crate::engine::tick::TickResult;
use crate::engine::zone::ZoneId;
use crate::engine::BrowsingContext;
use crate::render::backend::{
    CompositorSink, ErasedSurface, PresentMode, RenderBackend, RgbaImage, SurfaceSize,
};
use crate::render::Viewport;
use crate::{EngineCommand, EngineEvent};
use serde::__private::from_utf8_lossy;
use std::sync::Arc;
use std::time::Instant;
use tokio::runtime::Runtime;
use url::Url;
use uuid::Uuid;

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
    /// Create a new unique `TabId` using a random UUID.
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
    Rendering(Viewport),

    /// A new surface is ready for painting. The next `tick()` typically
    /// returns to [`TabState::Idle`] and sets `needs_redraw = true` in [`TickResult`].
    Rendered(Viewport),

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
/// A [`Tab`] owns an [`BrowsingContext`](crate::BrowsingContext) and tracks its
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
    /// Browsing context running for this tab
    pub context: BrowsingContext,
    /// State of the tab (idle, loading, loaded, etc.)
    pub state: TabState,

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

    /// Backend rendering
    pub thumbnail: Option<RgbaImage>, // Thumbnail image of the tab in case the tab is not visible
    surface: Option<Box<dyn ErasedSurface>>, // Surface on which the browsing context can render the tab
    surface_size: SurfaceSize, // Size of the surface (does not have to match viewport)
    present_mode: PresentMode, // Present mode for the surface?

    /// The viewport that was committed for the in-flight/last render
    committed_viewport: Viewport,
    /// The newest viewport requested by the tab, which may differ from the committed one.
    desired_viewport: Viewport,
    /// Set when a resize arrives while rendering. Causes an immediate re-render after finihsing the current rendering.
    dirty_after_inflight: bool,
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
        // surface_provider: Arc<dyn SurfaceProvider>,
        viewport: Viewport,
        cookie_jar: Option<CookieJarHandle>,
    ) -> Self {
        let mut tab = Self {
            id: TabId::new(),
            zone_id,
            state: TabState::Idle,
            context: BrowsingContext::new(runtime),

            favicon: vec![],              // Placeholder for favicon data
            title: "New Tab".to_string(), // Title of the new tab

            pending_url: None,
            current_url: None,
            is_loading: false,
            is_error: false,

            mode: TabMode::Active, // Default mode is active
            last_tick: Instant::now(),

            cookie_jar,
            partition_key: PartitionKey::None, // Start with no partition key
            partition_policy: PartitionPolicy::TopLevelOrigin,

            surface: None, // No surface initially
            surface_size: SurfaceSize {
                width: 1,
                height: 1,
            },
            present_mode: PresentMode::Fifo,
            thumbnail: None, // No thumbnail initially

            committed_viewport: viewport,
            desired_viewport: viewport,
            dirty_after_inflight: false,
        };

        tab.context.set_viewport(viewport);

        tab
    }

    /// Navigate to a URL (string is parsed into a `Url`). On success, moves the
    /// tab to [`TabState::PendingLoad(url)`]. Invalid URLs are ignored and logged.
    pub fn navigate_to(&mut self, url: impl Into<String>) {
        let url = match Url::parse(&url.into()) {
            Ok(url) => url,
            Err(e) => {
                // Can't parse string to a URL to load
                log::error!("Tab[{:?}]: Cannot parse URL: {}", self.id, e);
                return;
            }
        };

        self.state = TabState::PendingLoad(url.into());
        self.is_loading = true;
    }

    /// Bind local+session storage handles into the underlying browsing context.
    /// Call this after creating the tab or when the zone’s storage changes.
    pub fn bind_storage(&mut self, storage: StorageHandles) {
        self.context.bind_storage(storage.local, storage.session);
    }

    /// Set a new viewport and schedule a re-render
    /// by transitioning to [`TabState::PendingRendering`].
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.surface_size = SurfaceSize {
            width: viewport.width,
            height: viewport.height,
        };

        self.context.set_viewport(viewport);
        self.desired_viewport = viewport;

        if let TabState::Rendering(_) = self.state {
            // Mark the fact that we have triggered a resize during the rendering of the tab
            self.dirty_after_inflight = true;
        } else {
            self.state = TabState::PendingRendering(self.desired_viewport);
        }
    }

    /// Advance the tab’s state machine once and return a [`TickResult`]
    /// indicating whether a redraw is needed and whether a page was committed.
    ///
    /// **Returns**
    /// - `needs_redraw = true` when a new surface is ready to paint
    /// - `page_loaded = true` when a navigation commits
    pub(crate) fn tick(
        &mut self,
        backend: &mut dyn RenderBackend,
        host: &mut impl CompositorSink,
    ) -> anyhow::Result<TickResult> {
        let mut result = TickResult::default();

        match self.state.clone() {
            TabState::Idle => {
                // Nothing to do
            }

            // Start loading the URL
            TabState::PendingLoad(url) => {
                self.state = TabState::Loading;
                self.is_loading = true;
                self.pending_url = Some(url.clone());
                self.context.start_loading(url.clone());
            }

            // Poll the loading task until it's completed (or failed)
            TabState::Loading => {
                if let Some(done) = self.context.poll_loading() {
                    match done {
                        Ok(resp) => {
                            // Store cookies from the response in the cookie jar
                            if let Some(cookie_jar) = &self.cookie_jar {
                                cookie_jar
                                    .write()
                                    .unwrap()
                                    .store_response_cookies(&resp.url, &resp.headers);
                            }

                            // Set tab state
                            self.state = TabState::Loaded;
                            self.is_loading = false;
                            self.pending_url = None;
                            self.current_url = Some(resp.url.clone());
                            self.context
                                .set_raw_html(from_utf8_lossy(resp.body.as_slice()).as_ref());

                            // Set result
                            result.page_loaded = true;
                            result.commited_url = Some(resp.url.clone());
                        }
                        Err(e) => {
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
                self.state = TabState::PendingRendering(*self.context.viewport());
            }

            TabState::PendingRendering(_viewport) => {
                if self.committed_viewport != self.desired_viewport {
                    self.committed_viewport = self.desired_viewport;
                    self.surface_size = self.committed_viewport.as_size();
                }
                self.state = TabState::Rendering(self.committed_viewport);
            }

            // Normally, rendering will take a while (async). Currently, it doesn't so we move directly
            // to a Rendered state.
            TabState::Rendering(viewport) => {
                // Make sure we have a surface to render on
                self.ensure_surface(backend, viewport.as_size())?;

                // Rebuild the render list if needed
                self.context.rebuild_render_list_if_needed();

                if let Some(ref mut surf) = self.surface {
                    backend.render(&mut self.context, surf.as_mut())?;

                    if let Some(handle) = backend.external_handle(surf.as_mut()) {
                        host.submit_frame(self.id, handle);
                    }
                }

                self.state = TabState::Rendered(viewport);
            }

            // Notify the outside world that we have something to paint, and we can go back to idle state.
            TabState::Rendered(_viewport) => {
                // Tell the world our surface is ready to paint
                result.needs_redraw = true;

                if self.dirty_after_inflight || self.committed_viewport != self.desired_viewport {
                    // If we have a dirty viewport, we need to re-render it
                    self.dirty_after_inflight = false;
                    self.state = TabState::PendingRendering(self.desired_viewport);
                } else {
                    // If we are not dirty, we can go back to idle state
                    self.state = TabState::Idle;
                }

                self.state = TabState::Idle;
            }

            TabState::Failed(error_msg) => {
                // Something has failed. We need to show the error message so we set the raw HTML
                // to the error message and trigger a redraw.
                self.context.set_raw_html(error_msg.as_str());
                self.state = TabState::Loaded;

                result.needs_redraw = true;
            }
        }

        Ok(result)
    }

    /// Handle an external UI event (scroll, mouse, keyboard, resize).
    /// Typically forwarded from your toolkit.
    pub(crate) fn handle_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Scroll { dx, dy } => {
                let cur_vp = self.context.viewport();
                self.set_viewport(Viewport::new(
                    // We should do clamp(), but we don't know the max x/y sizes of the rendered document
                    (cur_vp.x + dx as i32).max(0),
                    (cur_vp.y + dy as i32).max(0),
                    cur_vp.width,
                    cur_vp.height,
                ));
            }
            EngineEvent::MouseMove { x, y } => {
                println!(
                    "Mouse moved on tab {:?} to position ({}, {})",
                    self.id, x, y
                );
            }
            EngineEvent::MouseDown { button, x, y } => {
                println!(
                    "Mouse down event on tab {:?} at position ({}, {}) with button {:?}",
                    self.id, x, y, button
                );
            }
            EngineEvent::MouseUp { button, x, y } => {
                println!(
                    "Mouse up event on tab {:?} at position ({}, {}) with button {:?}",
                    self.id, x, y, button
                );
            }
            EngineEvent::KeyDown { key } => {
                println!("Key down event on tab {:?} for key: {}", self.id, key);
            }
            EngineEvent::KeyUp { key } => {
                println!("Key up event on tab {:?} for key: {}", self.id, key);
            }
            EngineEvent::InputChar { character } => {
                println!(
                    "Input character event on tab {:?}: '{}'",
                    self.id, character
                );
            }
            EngineEvent::Resize { width, height } => {
                println!(
                    "Resize event on tab {:?}: new size {}x{}",
                    self.id, width, height
                );
                let cur_vp = self.context.viewport();
                self.set_viewport(Viewport::new(cur_vp.x, cur_vp.y, width, height))
            }
        }
    }

    /// Execute a high-level engine command (navigate, reload).
    pub(crate) fn execute_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::Navigate(url) => {
                self.state = TabState::PendingLoad(url);
            }
            EngineCommand::Reload() => {
                let Some(url) = self.current_url.clone() else {
                    return;
                };

                self.state = TabState::PendingLoad(url);
            }
        }
    }

    /// Get the current snapshotted image of the tab.
    pub fn thumbnail(&self) -> Option<&RgbaImage> {
        self.thumbnail.as_ref()
    }

    /// Dispatch a storage event to same-origin documents in this tab (placeholder).
    /// Intended for HTML5 storage event semantics.
    pub(crate) fn dispatch_storage_event_to_same_origin_docs(
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

    /// Ensure the tab has a surface of the given size, creating it if necessary.
    fn ensure_surface(
        &mut self,
        backend: &dyn RenderBackend,
        size: SurfaceSize,
    ) -> anyhow::Result<()> {
        if let Some(ref surf) = self.surface {
            if surf.size() == size {
                return Ok(());
            }
        }
        self.surface = Some(backend.create_surface(size, self.present_mode)?);
        Ok(())
    }
}
