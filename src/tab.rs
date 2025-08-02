use std::sync::Arc;
use std::time::Instant;
use gtk4::cairo;
use tokio::runtime::Runtime;
use uuid::Uuid;
use crate::instance::EngineInstance;
use crate::event::EngineEvent;
use crate::tick::TickResult;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TabId(Uuid);

impl TabId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabState {
    Idle,
    PendingLoad(String),
    Loading,
    Loaded,
    Rendering,
    Rendered,
    Failed(String),
}

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

pub struct TabMeta {}

pub struct Tab {
    pub id: TabId,                  // ID of the tab
    pub instance: EngineInstance,   // Engine instance running for this tab
    pub state: TabState,            // State of the tab (idle, loading, loaded, etc.)

    // Scheduling and lifecycle management
    pub mode: TabMode,              // Current tab mode (idle, live, background)
    pub last_tick: Instant,         // When was the last tick?

    pub favicon: Vec<u8>,               // Favicon binary data for the current tab
    pub title: String,                  // Title of the current tab

    pub is_loading: bool,               // Is the current URL being loaded
    pub is_error: bool,                 // Is there an error in the current tab?
}

impl Tab {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            id: TabId::new(),
            state: TabState::Idle,
            instance: EngineInstance::new(runtime),

            favicon: vec![],                    // Placeholder for favicon data
            title: "New Tab".to_string(),       // Title of the new tab

            is_loading: false,
            is_error: false,

            mode: TabMode::Active,              // Default mode is active
            last_tick: Instant::now(),
        }
    }

    pub fn tick(&mut self) -> TickResult {
        let mut result = TickResult::default();

        match self.state.clone() {
            TabState::Idle => {
                // Nothing to do
            }

            TabState::PendingLoad(url) => {
                self.instance.start_loading(url);
                self.state = TabState::Loading;
            }

            TabState::Loading => {
                if let Some(done) = self.instance.poll_loading() {
                    match done {
                        Ok(html) => {
                            self.state = TabState::Loaded;
                            self.instance.set_raw_html(html);
                            result.page_loaded = true;
                        }
                        Err(e) => {
                            self.state = TabState::Failed(e);
                            result.needs_redraw = true;
                        }
                    }
                }
            }

            TabState::Loaded => {
                println!("Tabstate loaded, starting rendering");
                self.instance.start_rendering();
                self.state = TabState::Rendering;
            }

            TabState::Rendered => {
                self.state = TabState::Idle;
                result.needs_redraw = true;
            }

            TabState::Failed(msg) => {
                self.instance.render_error(&msg);
                self.state = TabState::Rendered;
                result.needs_redraw = true;
            }
            TabState::Rendering => {
                self.state = TabState::Rendered;
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
            }
            EngineEvent::LoadUrl(url) => {
                println!("Loading URL '{}' in tab {:?}", url, self.id);
                // self.pending_url = Some(url.clone());f
                self.state = TabState::PendingLoad(url);
            }
        }
    }

    pub fn get_surface(&self) -> Option<&cairo::ImageSurface> {
        self.instance.surface()
    }
}