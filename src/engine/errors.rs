/// Public engine errors available for the outside world
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    /// An invalid tab ID has been provided.
    #[error("Invalid tab ID")]
    InvalidTabId,

    /// An invalid zone ID has been provided.
    #[error("Invalid zone ID")]
    InvalidZoneId,

    /// The zone manager cannot create any more zones (limit reached)
    #[error("Zone limit exceeded")]
    ZoneLimitExceeded,

    /// A network error has occurred
    #[error("Network error: {0}")]
    NetworkError(String),

    /// A parser error has occurred
    #[error("Parser error: {0}")]
    ParserError(String),

    /// A rendering error has occurred
    #[error("Renderer error: {0}")]
    RendererError(String),

    /// Some internal issue within the engine has occurred
    #[error("Internal engine error")]
    Internal,

    /// The zone provided by the zone id is not found (permissions or does not exist)
    #[error("Zone not found")]
    ZoneNotFound,

    /// The zone is already locked (e.g., trying to modify a locked zone)
    #[error("Zone is already locked")]
    ZoneLocked,

    /// The number of tabs for this zone has been exceeded
    #[error("Tab limit in zone exceeded")]
    TabLimitExceeded,

    /// The zone id already exists
    #[error("Zone already exists")]
    ZoneAlreadyExists,

    /// An invalid configuration was provided for the engine or zone
    #[error("Invalid configuration: {0}")]
    InvalidConfiguration(String),

    /// Task/Tab creation failed
    #[error("Task init failed: {0}")]
    TaskInitFailed(#[source] anyhow::Error),

    #[error("poisoned")]
    Poisoned,

    #[error("Failed to create tab: {0}")]
    CreateTab(#[source] anyhow::Error),

    #[error("Channel closed")]
    ChannelClosed,

    #[error("Failed to create zone: {0}")]
    CreateZone(#[source] anyhow::Error),

    #[error("Engine is already running")]
    AlreadyRunning,

    #[error("Engine is not running")]
    NotRunning,
}
