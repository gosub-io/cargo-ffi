use url::Url;

#[derive(Debug, Clone)]
pub enum MouseButton {
    /// Left mouse button pressed (or depressed)
    Left,
    /// Middle mouse button pressed (or depressed)
    Middle,
    /// Right mouse button pressed (or depressed)
    Right,
}

/// Events that have occurred and must be passed to the engine from the user agent
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Move has moved to a new position
    MouseMove{ x: f32, y: f32 },
    /// A mouse button was pressed
    MouseDown{ button: MouseButton, x: f32, y: f32 },
    /// A mouse button was released
    MouseUp{ button: MouseButton, x: f32, y: f32 },
    /// Viewport has been scrolled (with delta coordinates)
    Scroll{ dx: f32, dy: f32 },
    /// A key was pressed
    KeyDown{ key: String },
    /// A key was released
    KeyUp{ key: String },
    /// A character was input (like a letter in an input box)
    InputChar{ character: char },
    /// A resize event occurred
    Resize{ width: u32, height: u32 },
}

/// Commands that the engine need to execute
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// An url must be loaded inside the tab
    Navigate(Url),
    /// Reload the current URL in the tab
    Reload(),
}