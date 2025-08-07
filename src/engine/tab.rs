use std::sync::Arc;
use std::time::Instant;
use gtk4::cairo;
use tokio::runtime::Runtime;
use uuid::Uuid;
use crate::{EngineCommand, EngineEvent, EngineInstance};
use crate::tick::TickResult;
use crate::viewport::Viewport;
use crate::zone::zone::ZoneId;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    /// Tab is not doing anything and does not have timers or animations
    Idle,
    /// Tab is currently triggered to load an URL
    PendingLoad(String),
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

// Tab mode defines its activity. Based on this, it will get a certain slice of time for processing.
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

pub struct Tab {
    pub id: TabId,                  // ID of the tab
    pub zone_id: ZoneId,            // ID of the zone in which this tab resides
    pub instance: EngineInstance,   // Engine instance running for this tab
    pub state: TabState,            // State of the tab (idle, loading, loaded, etc.)

    pub viewport: Viewport,         // Current (or wanted) viewport for rendering

    // Scheduling and lifecycle management
    pub mode: TabMode,              // Current tab mode (idle, live, background)
    pub last_tick: Instant,         // When was the last tick?

    pub favicon: Vec<u8>,               // Favicon binary data for the current tab
    pub title: String,                  // Title of the current tab

    pub current_url: String,            // Current URL that is loaded or being loadeds
    pub is_loading: bool,               // Is the current URL being loaded
    pub is_error: bool,                 // Is there an error in the current tab?
}

impl Tab {
    pub fn new(zone_id: ZoneId, runtime: Arc<Runtime>, viewport: &Viewport) -> Self {
        Self {
            id: TabId::new(),
            zone_id,
            state: TabState::Idle,
            instance: EngineInstance::new(runtime),
            viewport: viewport.clone(),

            favicon: vec![],                    // Placeholder for favicon data
            title: "New Tab".to_string(),       // Title of the new tab

            current_url: "".to_string(),
            is_loading: false,
            is_error: false,

            mode: TabMode::Active,              // Default mode is active
            last_tick: Instant::now(),
        }
    }

    pub fn set_viewport(&mut self, viewport: Viewport) {
        self.viewport = viewport;
        self.state = TabState::PendingRendering(self.viewport.clone())
    }

    pub fn tick(&mut self) -> TickResult {
        let mut result = TickResult::default();

        match self.state.clone() {
            TabState::Idle => {
                // Nothing to do
            }

            // Start loading the URL
            TabState::PendingLoad(url) => {
                self.instance.start_loading(url.clone());
                self.state = TabState::Loading;
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());
                self.is_loading = true;
                self.current_url = url;
            }

            // Poll the loading task until it's completed (or failed)
            TabState::Loading => {
                if let Some(done) = self.instance.poll_loading() {
                    match done {
                        Ok(html) => {
                            self.state = TabState::Loaded;
                            println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

                            self.instance.set_raw_html(html);
                            self.is_loading = false;
                            result.page_loaded = true;
                        }
                        Err(e) => {
                            self.state = TabState::Failed(e);
                            println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

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

    pub(crate) fn handle_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Scroll { dx, dy } => {
                println!("Scrolling tab {:?} by dx: {}, dy: {}", self.id, dx, dy);
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
                self.set_viewport(Viewport::new(width, height))
            }
        }
    }

    pub fn execute_command(&mut self, command: EngineCommand) {
        match command {
            EngineCommand::LoadUrl(url) => {
                println!("Loading URL '{}' in tab {:?}", url, self.id);
                // self.pending_url = Some(url.clone());f
                self.state = TabState::PendingLoad(url);
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

            }
            EngineCommand::Reload() => {
                let url = self.current_url.clone();
                println!("Reloading URL '{}' in tab {:?}", url, self.id);
                // self.pending_url = Some(url.clone());f
                self.state = TabState::PendingLoad(url);
                println!("Tab[{:?}]: State: {:?}\n", self.id, self.state.clone());

            }
        }
    }

    pub fn get_surface(&self) -> Option<&cairo::ImageSurface> {
        self.instance.surface()
    }
}