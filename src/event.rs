#[derive(Debug, Clone)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    MouseMove{ x: f32, y: f32 },
    MouseDown{ button: MouseButton, x: f32, y: f32 },
    MouseUp{ button: MouseButton, x: f32, y: f32 },
    Scroll{ dx: f32, dy: f32 },
    KeyDown{ key: String },
    KeyUp{ key: String },
    InputChar{ character: char },
    Resize{ width: u32, height: u32 },
    LoadUrl(String),
}