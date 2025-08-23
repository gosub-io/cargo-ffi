use url::Url;

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

/// Events that have occurred and must be passed to the engine from the user agent
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Move has moved to a new position
    MouseMove {
        /// The x coordinate of the mouse position
        x: f32,
        /// The y coordinate of the mouse position
        y: f32,
    },
    /// A mouse button was pressed
    MouseDown {
        /// The mouse button that was pressed
        button: MouseButton,
        /// The x coordinate of the mouse position when the button was pressed
        x: f32,
        /// The y coordinate of the mouse position when the button was pressed
        y: f32,
    },
    /// A mouse button was released
    MouseUp {
        /// The mouse button that was released
        button: MouseButton,
        /// The x coordinate of the mouse position when the button was released
        x: f32,
        /// The y coordinate of the mouse position when the button was released
        y: f32,
    },
    /// Viewport has been scrolled (with delta coordinates)
    Scroll {
        /// The x coordinate of the scroll delta
        dx: f32,
        /// The y coordinate of the scroll delta
        dy: f32,
    },
    /// A key was pressed
    KeyDown {
        /// The key that was pressed
        key: String,
    },
    /// A key was released
    KeyUp {
        /// The key that was released
        key: String,
    },
    /// A character was input (like a letter in an input box)
    InputChar {
        /// The character that was input
        character: char,
    },
    /// A resize event occurred
    Resize {
        /// The new width of the viewport
        width: u32,
        /// The new height of the viewport
        height: u32,
    },
}

/// Commands that the engine need to execute
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// An url must be loaded inside the tab
    Navigate(Url),
    /// Reload the current URL in the tab
    Reload(),
}
