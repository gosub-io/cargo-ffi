use tokio::sync::oneshot;
use url::Url;
use crate::config::LogLevel;
use crate::cookies::Cookie;
use crate::render::backend::ExternalHandle;
use crate::render::Viewport;
use crate::storage::event::StorageScope;
use crate::tab::{TabHandle, TabId};
use crate::zone::ZoneId;

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

pub enum ZoneCommand {
    SetTitle {
        zone: ZoneId,
        title: String,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    SetIcon {
        zone: ZoneId,
        icon: Vec<u8>,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    SetDescription {
        zone: ZoneId,
        description: String,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    SetColor {
        zone: ZoneId,
        color: [u8; 4],
        reply: oneshot::Sender<anyhow::Result<()>>,
    },

    OpenTab {
        zone: ZoneId,
        title: String,
        viewport: Viewport,
        reply: oneshot::Sender<anyhow::Result<TabHandle>>,
    },
    CloseTab {
        zone: ZoneId,
        tab: TabId,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ListTabs {
        zone: ZoneId,
        reply: oneshot::Sender<anyhow::Result<Vec<TabId>>>,
    },
    TabTitle {
        zone: ZoneId,
        tab: TabId,
        reply: oneshot::Sender<anyhow::Result<Option<String>>>,
    },
}

impl Debug for ZoneCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ZoneCommand::SetTitle { zone, title, .. } => f
                .debug_struct("SetTitle")
                .field("zone", zone)
                .field("title", title)
                .finish(),
            ZoneCommand::SetIcon { zone, icon, .. } => f
                .debug_struct("SetIcon")
                .field("zone", zone)
                .field("icon_len", &icon.len())
                .finish(),
            ZoneCommand::SetDescription { zone, description, .. } => f
                .debug_struct("SetDescription")
                .field("zone", zone)
                .field("description", description)
                .finish(),
            ZoneCommand::SetColor { zone, color, .. } => f
                .debug_struct("SetColor")
                .field("zone", zone)
                .field("color", color)
                .finish(),
            ZoneCommand::OpenTab { zone, title, viewport, .. } => f
                .debug_struct("OpenTab")
                .field("zone", zone)
                .field("title", title)
                .field("viewport", viewport)
                .finish(),
            ZoneCommand::CloseTab { zone, tab, .. } => f
                .debug_struct("CloseTab")
                .field("zone", zone)
                .field("tab", tab)
                .finish(),
            ZoneCommand::ListTabs { zone, .. } => f
                .debug_struct("ListTabs")
                .field("zone", zone)
                .finish(),
            ZoneCommand::TabTitle { zone, tab, .. } => f
                .debug_struct("TabTitle")
                .field("zone", zone)
                .field("tab", tab)
                .finish(),
        }
    }
}

#[derive(Debug)]
pub enum EngineCommand {
    // Zone management
    Zone(ZoneCommand),

    // Navigation / lifecycle
    Navigate { url: Url },
    Reload { ignore_cache: bool },
    StopLoading,
    CloseTab,

    // Rendering control
    ResumeDrawing { fps: u16 },
    SuspendDrawing,
    Resize { width: u32, height: u32 },
    SetViewport { x: i32, y: i32, width: u32, height: u32 },

    // User input
    MouseMove { x: f32, y: f32 },
    MouseDown { x: f32, y: f32, button: MouseButton },
    MouseUp { x: f32, y: f32, button: MouseButton },
    MouseScroll { delta_x: f32, delta_y: f32 },
    KeyDown { key: String, code: String, modifiers: Modifiers },
    KeyUp { key: String, code: String, modifiers: Modifiers },
    TextInput { text: String },

    // Session / zone state
    SetCookie { cookie: Cookie },
    ClearCookies,
    SetStorageItem { key: String, value: String },
    RemoveStorageItem { key: String },
    ClearStorage,

    // Media / scripting
    ExecuteScript { source: String },
    PlayMedia { element_id: u64 },
    PauseMedia { element_id: u64 },

    // Debug / devtools
    EnableLogging { level: LogLevel },
    DumpDomTree,
    InputChar,
}


#[derive(Debug)]
pub enum EngineEvent {
    // Rendering
    Redraw { tab: TabId, handle: ExternalHandle },
    FrameComplete { tab: TabId, frame_id: u64 },

    TitleChanged { tab: TabId, title: String },
    FavIconChanged { tab: TabId, favicon: Vec<u8> },
    LocationChanged { tab: TabId, url: Url },

    // Navigation
    ConnectionEstablished { tab: TabId, url: Url },
    Redirect { tab: TabId, from: Url, to: Url },
    LoadStarted { tab: TabId, url: Url },
    LoadProgress { tab: TabId, progress: f32 },
    LoadFinished { tab: TabId, url: Url },
    LoadFailed { tab: TabId, url: Url, error: String },
    PageCommitted { tab: TabId, url: Url },

    // Tab lifecycle
    TabCreated { tab: TabId },
    TabClosed { tab: TabId },

    // Input / interaction
    FocusChanged { tab: TabId, focused: bool },
    // CursorChanged { tab: TabId, cursor: CursorIcon },
    KeyDown { key: String, code: String, modifiers: Modifiers },
    KeyUp { key: String, code: String, modifiers: Modifiers },
    MouseUp { button: MouseButton, x: f32, y: f32 },
    MouseDown { button: MouseButton, x: f32, y: f32 },
    InputChar { character: char },

    // Session / zone state
    CookieAdded { tab: TabId, cookie: Cookie },
    StorageChanged {
        tab: Option<TabId>,
        zone: Option<ZoneId>,
        key: String,
        value: Option<String>,
        scope: StorageScope,
        origin: url::Origin
    },

    // Media / scripting
    MediaStarted { tab: TabId, element_id: u64 },
    MediaPaused { tab: TabId, element_id: u64 },
    ScriptResult { tab: TabId, result: serde_json::Value },

    // Errors / diagnostics
    NetworkError { tab: TabId, url: Url, message: String },
    JavaScriptError { tab: TabId, message: String, line: u32, column: u32 },
    EngineCrashed { tab: TabId, reason: String },

    // Uncategorized / generic


}