#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Invalid tab ID")]
    InvalidTabId,

    #[error("Invalid zone ID")]
    InvalidZoneId,

    #[error("Zone limit exceeded")]
    ZoneLimitExceeded,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Parser error: {0}")]
    ParserError(String),

    #[error("Renderer error: {0}")]
    RendererError(String),

    #[error("Internal engine error")]
    Internal,

    #[error("Zone not found")]
    ZoneNotFound,

    #[error("Zone is already locked")]
    ZoneLocked,

    #[error("Tab limit in zone exceeded")]
    TabLimitExceeded,
}