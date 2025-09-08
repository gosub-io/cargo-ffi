/// Defines the different resource types that are available for loading
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ResourceKind {
    Document,
    Stylesheet,
    Script,
    Image,
    Font,
    Media,
    Xhr,
    Fetch,
    WebSocket,
    Other,
}

/// Defines who initiated the resource load
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Initiator {
    /// Initiated by the user, UI, or link click
    Navigation,
    /// HTML Parser resource
    Parser,
    /// Initiated by a JS script (or Lua script) (fetch, XHR)
    Script,
    /// CSS @import, font-face
    CSS,
    /// Other undefined type of initiator
    Other,
}