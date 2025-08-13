// src/engine/tab.rs
//! Tab system: [`Tab`], ['Tick'], and [`TabId`].
//!

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
use crate::engine::storage::{Origin, PartitionKey, StorageEvent, StorageHandles};
use crate::engine::storage::types::PartitionPolicy;
use crate::engine::zone::ZoneId;

/// A unique identifier for a tab, represented as a UUID.
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
    /// Tab is not doing anything and does not have timers or animations
    #[default]
    Idle,
    /// Tab is currently triggered to load an URL
    PendingLoad(Url),
    /// Tab is currently loading an URL
    Loading,
    /// Tab has loaded the URL
    Loaded,
    /// Render for a new viewport
    PendingRendering(Viewport),
    /// Tab is currently rendering a new surface
    Rendering,
    /// New surface has been rendered
    Rendered,
    /// Something failed in the tab
    Failed(String),
}

/// Tab mode defines its activity. Based on this, it will get a certain slice of time for processing.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TabMode {
    /// Fully active: network, animations, layout, painting at 60 Hz
    Active,
    /// “Live” background: keep CSS animations & timers at, say, 10 Hz
    BackgroundLive,
    /// “Sleeping” background: only tick network / JS timers at 1 Hz
    BackgroundIdle,
    /// Completely suspended: no ticking until an event or visibility change
    Suspended,
}


/// A tab is a single instance of a web page that is being rendered in a zone. It has its own
/// viewport, engine instance etc. You could have 2 tabs open in a single window, like split-screen.
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

    pub fn navigate_to(&mut self, url: impl Into<String>) {
        let url = match Url::parse(&url.into()) {
            Ok(url) => url,
            Err(_) => {
                // Can't parse string to a URL to load
                eprintln!("Invalid URL");
                return
            }
        };

        self.state = TabState::PendingLoad(url.into());
        self.is_loading = true;
    }

    /// Bind storage handles into the engine instance
    pub fn bind_storage(&mut self, storage: StorageHandles) {
        self.instance.bind_storage(storage.local, storage.session);
    }

    // Set the viewport for the tab. This will trigger a re-rendering of the tab.
    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        self.state = TabState::PendingRendering(self.viewport.clone())
    }

    // Tick the tab. This will process the current state of the tab and return a TickResult.
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

    /// Handle an external (UA) eventf for this tab
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

    /// Executes an engine command on the tab
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

    /// Retrieves the surface allocated for this tab
    pub fn get_surface(&self) -> Option<&cairo::ImageSurface> {
        self.instance.surface()
    }

    pub fn dispatch_storage_event_to_same_origin_docs(
        &mut self,
        _origin: &Origin,
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