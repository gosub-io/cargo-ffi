use std::fmt::Debug;
use tokio::sync::{broadcast, oneshot};
use url::Url;
use crate::config::LogLevel;
use crate::cookies::Cookie;
use crate::EngineError;
use crate::render::backend::ExternalHandle;
use crate::storage::event::StorageScope;
use crate::tab::{TabDefaults, TabHandle, TabId, TabOverrides};
use crate::zone::{Zone, ZoneConfig, ZoneId, ZoneServices};

/// Represents a mouse button that can be pressed or released
#[derive(Debug, Clone)]
pub enum MouseButton {
    /// Left mouse button pressed (or depressed)
    Left,
    /// Middle mouse button pressed (or depressed)
    Middle,
    /// Right mouse button pressed (or depressed)
    Right,
}

/// Represents modifier keys that can be held down during keyboard events
#[derive(Debug, Clone)]
pub enum Modifiers {
    /// The Shift key is held down
    Shift,
    /// The Control key is held down
    Control,
    /// The Alt key is held down
    Alt,
    /// The Meta key (Command on Mac, Windows key on Windows) is held down
    Meta,
}

/// Commands related to zone management
pub enum ZoneCommand {
    /// Set the title of the zone
    SetTitle {
        zone_id: ZoneId,
        title: String,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    /// Set the icon of the zone (favicon)
    SetIcon {
        zone_id: ZoneId,
        icon: Vec<u8>,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    /// Set the description of the zone
    SetDescription {
        zone_id: ZoneId,
        description: String,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    /// Set the color of the zone (RGBA)
    SetColor {
        zone_id: ZoneId,
        color: [u8; 4],
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    /// Open a tab in the zone
    CreateTab {
        zone_id: ZoneId,
        initial: TabDefaults,
        overrides: Option<TabOverrides>,
        reply: oneshot::Sender<anyhow::Result<TabHandle, EngineError>>,
    },
    /// Close a tab in the zone
    CloseTab {
        zone_id: ZoneId,
        tab_id: TabId,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    /// List all tabs in the zone
    ListTabs {
        zone_id: ZoneId,
        reply: oneshot::Sender<anyhow::Result<Vec<TabId>>>,
    },
}

impl Debug for ZoneCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneCommand::SetTitle { zone_id, title, .. } => f
                .debug_struct("SetTitle")
                .field("zone_id", zone_id)
                .field("title", title)
                .finish(),
            ZoneCommand::SetIcon { zone_id, icon, .. } => f
                .debug_struct("SetIcon")
                .field("zone_id", zone_id)
                .field("icon_len", &icon.len())
                .finish(),
            ZoneCommand::SetDescription { zone_id, description, .. } => f
                .debug_struct("SetDescription")
                .field("zone_id", zone_id)
                .field("description", description)
                .finish(),
            ZoneCommand::SetColor { zone_id, color, .. } => f
                .debug_struct("SetColor")
                .field("zone_id", zone_id)
                .field("color", color)
                .finish(),
            ZoneCommand::CreateTab { zone_id,  .. } => f
                .debug_struct("OpenTab")
                .field("zone_id", zone_id)
                .finish(),
            ZoneCommand::CloseTab { zone_id, tab_id, .. } => f
                .debug_struct("CloseTab")
                .field("zone_id", zone_id)
                .field("tab_id", tab_id)
                .finish(),
            ZoneCommand::ListTabs { zone_id, .. } => f
                .debug_struct("ListTabs")
                .field("zone_id", zone_id)
                .finish(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum TabCommand {
    // ** Navigation / lifecycle
    /// Navigate to specific URL
    Navigate { url: String },
    /// Reload current URL (with or without cache)
    Reload { ignore_cache: bool },
    /// Cancel the current load (if any)
    StopLoading,
    /// Close tab
    CloseTab,

    // ** Rendering control
    /// Resume sending draw events to the tab's event channel. Use fps as the refresh limit
    ResumeDrawing { fps: u16 },
    /// Suspend sending draw events
    SuspendDrawing,
    /// Resize viewport
    Resize { width: u32, height: u32 },
    /// Set viewport
    SetViewport { x: i32, y: i32, width: u32, height: u32 },

    // ** User input
    /// Mouse moved to new position
    MouseMove { x: f32, y: f32 },
    /// Mouse button is pressed
    MouseDown { x: f32, y: f32, button: MouseButton },
    /// Mouse button is depressed
    MouseUp { x: f32, y: f32, button: MouseButton },
    /// Mouse scrolled up by delta
    MouseScroll { delta_x: f32, delta_y: f32 },
    /// Key has been pressed
    KeyDown { key: String, code: String, modifiers: Modifiers },
    /// Key has been depressed
    KeyUp { key: String, code: String, modifiers: Modifiers },
    /// Text input
    TextInput { text: String },
    /// Char input (@TODO: Needed since we have TextInput)?
    CharInput { ch: char },

    // ** Session / zone state
    /// Set a specific cookie
    SetCookie { cookie: Cookie },
    /// Clear all cookies
    ClearCookies,
    /// Set storage item (@TODO: local / session??)
    SetStorageItem { key: String, value: String },
    /// Remove storage item
    RemoveStorageItem { key: String },
    /// Clear whole storage
    ClearStorage,

    // ** Media / scripting
    /// Execute given javascript (how about lua?)
    ExecuteScript { source: String },
    /// Play media in element_id
    PlayMedia { element_id: u64 },
    /// Pause media in element_id
    PauseMedia { element_id: u64 },

    // ** Debug / devtools
    /// Enable logging
    EnableLogging { level: LogLevel },
    /// Dump dom tree
    DumpDomTree,
}

#[derive(Debug)]
pub enum EngineCommand {
    // ** Engine control

    /// Gracefully shutdown the engine
    Shutdown{ reply: oneshot::Sender<anyhow::Result<(), EngineError>> },

    // ** Zone management
    /// Create a new zone
    CreateZone{
        config: ZoneConfig,
        services: ZoneServices,
        zone_id: Option<ZoneId>,
        event_tx: broadcast::Sender<EngineEvent>,
        reply: oneshot::Sender<anyhow::Result<Zone, EngineError>>
    },
    /// Destroy a zone
    DestroyZone{
        zone_id: ZoneId,
        reply: oneshot::Sender<anyhow::Result<(), EngineError>>
    },

    /// Send a command to a specific zone
    // Zone(ZoneCommand),

    // ** Debug / devtools
    /// Enable logging
    EnableLogging { level: LogLevel },
}


#[derive(Debug, Clone)]
pub enum EngineEvent {
    // ** Engine lifecycle
    /// Engine has started
    EngineStarted,
    BackendChanged { old: String, new: String },
    /// Warning from the engine
    Warning { message: String },
    /// Engine is shutting down
    EngineShutdown,


    ZoneCreated { zone_id: ZoneId },
    ZoneClosed { zone_id: ZoneId },


    // ** Rendering
    /// A redraw frame is available
    Redraw { tab_id: TabId, handle: ExternalHandle },
    /// Frame has been completed (@TODO: do we need this?)
    FrameComplete { tab_id: TabId, frame_id: u64 },

    /// Title of the tab has changed
    TitleChanged { tab_id: TabId, title: String },
    /// Favicon of tab has changed
    FavIconChanged { tab_id: TabId, favicon: Vec<u8> },
    /// Location of the tab has changed
    LocationChanged { tab_id: TabId, url: Url },

    // ** Navigation
    /// Network connection has been established
    ConnectionEstablished { tab_id: TabId, url: Url },
    /// Redirect occurred
    Redirect { tab_id: TabId, from: Url, to: Url },
    /// Loading of the HTML started
    LoadStarted { tab_id: TabId, url: Url },
    /// Progress of loading
    LoadProgress { tab_id: TabId, progress: f32 },
    /// Loading of the HTML has finished
    LoadFinished { tab_id: TabId, url: Url },
    /// Loading has failed
    LoadFailed { tab_id: TabId, url: Url, error: String },
    /// Page committed (@TODO: needed? what for?)
    PageCommitted { tab_id: TabId, url: Url },

    // ** Tab lifecycle
    /// New tab created in zone
    TabCreated { tab_id: TabId, zone_id: ZoneId },
    /// Tab closed in zone
    TabClosed { tab_id: TabId, zone_id: ZoneId },

    // ** Input / interaction
    // FocusChanged { tab: TabId, focused: bool },
    // // CursorChanged { tab: TabId, cursor: CursorIcon },
    // KeyDown { key: String, code: String, modifiers: Modifiers },
    // KeyUp { key: String, code: String, modifiers: Modifiers },
    // MouseUp { button: MouseButton, x: f32, y: f32 },
    // MouseDown { button: MouseButton, x: f32, y: f32 },
    // InputChar { character: char },
    // InputText { character: char },

    // ** Session / zone state
    /// A cookie has been added
    CookieAdded { tab_id: TabId, cookie: Cookie },
    /// Storage has changed
    StorageChanged {
        tab_id: Option<TabId>,
        zone: Option<ZoneId>,
        key: String,
        value: Option<String>,
        scope: StorageScope,
        origin: url::Origin
    },

    // Media / scripting
    /// Media has started
    MediaStarted { tab_id: TabId, element_id: u64 },
    /// Media has paused
    MediaPaused { tab_id: TabId, element_id: u64 },
    /// Result of a script is returned (console stuff?)
    ScriptResult { tab_id: TabId, result: serde_json::Value },

    // Errors / diagnostics
    /// Network error occurred
    NetworkError { tab_id: TabId, url: Url, message: String },
    /// Javascript (parse) error
    JavaScriptError { tab_id: TabId, message: String, line: u32, column: u32 },
    /// Engine crashed
    TabCrashed { tab_id: TabId, reason: String },

    // Uncategorized / generic
}