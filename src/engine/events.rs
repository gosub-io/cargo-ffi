use url::Url;
use crate::config::LogLevel;
use crate::cookies::Cookie;
use crate::render::backend::ExternalHandle;
use crate::storage::event::StorageScope;
use crate::tab::TabId;
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

#[derive(Debug)]
pub enum EngineCommand {
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
    MouseDown { button: MouseButton },
    MouseUp { button: MouseButton },
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

    // Input / interaction
    FocusChanged { tab: TabId, focused: bool },
    // CursorChanged { tab: TabId, cursor: CursorIcon },

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
}