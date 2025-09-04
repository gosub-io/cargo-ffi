use std::sync::Arc;
use tokio::sync::{mpsc, broadcast};
use tokio_util::sync::CancellationToken;
use url::Url;
use crate::cookies::CookieJarHandle;
use crate::engine::events::EngineEvent;
use crate::events::TabCommand;
use crate::net::Response;
use crate::render::Viewport;
use crate::storage::{PartitionKey, StorageService};
use crate::tab::TabId;

/// Represents an in-flight network load operation. It allows for easy cancellation in case
/// the load is no longer needed (e.g., user navigated away).
#[allow(unused)]
pub(crate) struct InflightLoad {
    cancel: CancellationToken,
    rx: tokio::sync::oneshot::Receiver<anyhow::Result<Response>>,
}

/// Arguments required to spawn a new tab task.
#[derive(Debug)]
pub struct TabSpawnArgs {
    /// Tab I
    pub tab_id: TabId,
    /// Receive channel for commands for the tab
    pub cmd_rx: mpsc::Receiver<TabCommand>,
    /// Send channel for events from the tab to the UA
    pub event_tx: broadcast::Sender<EngineEvent>,
    /// Services available to the tab
    pub services: EffectiveTabServices,
    // Handle to the engine for shared resources
    // pub engine: EngineHandle,
    // Initial parameters for the tab
    // pub initial: TabDefaults,
}

/// Current state of the tab. This is a state machine that defines what the tab is doing at the moment.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
#[allow(unused)]
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
#[allow(unused)]
pub enum TabActivityMode {
    /// Foreground: fully active (network, layout, paint, animations ~60 Hz).
    Active,

    /// Background with animations alive but throttled (e.g., ~10 Hz).
    BackgroundLive,

    /// Background with minimal ticking (network/JS timers only, e.g., ~1 Hz).
    BackgroundIdle,

    /// Suspended: no ticking until an event or visibility change.
    Suspended,
}


// /// Parameters for creating a new tab.
// ///
// /// This lets the UA specify an optional title, viewport size, and/or URL
// /// when calling `ZoneHandle::create_tab()`.
// #[derive(Debug, Clone)]
// pub struct OpenTabParams {
//     /// Optional initial title (e.g. "New Tab").
//     /// The engine will later override this when a document sets `<title>`.
//     pub title: Option<String>,
//     /// Optional viewport to use at creation.
//     /// If `None`, the tab starts with a default and can be updated later
//     /// by calling `TabHandle::set_viewport()`.
//     pub viewport: Option<Viewport>,
//     /// Optional URL string to navigate to (like `about:blank` or a real page).
//     pub url: Option<String>,
// }
//
// impl Default for OpenTabParams {
//     fn default() -> Self {
//         Self {
//             title: Some("New Tab".to_string()),
//             viewport: None,
//             url: None,
//         }
//     }
// }


/// The effective services for a tab after applying zone defaults and tab overrides.
#[derive(Clone, Debug)]
pub struct EffectiveTabServices {
    pub partition_key: PartitionKey,
    pub storage: Arc<StorageService>,
    pub cookie_jar: CookieJarHandle,
}
