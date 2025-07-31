#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("Invalid tab ID")]
    InvalidTabId,

    #[error("Invalid group ID")]
    InvalidGroupId,

    #[error("Group limit exceeded")]
    GroupLimitExceeded,

    #[error("Network error: {0}")]
    NetworkError(String),

    #[error("Parser error: {0}")]
    ParserError(String),

    #[error("Renderer error: {0}")]
    RendererError(String),

    #[error("Internal engine error")]
    Internal,

    #[error("Group not found")]
    GroupNotFound,

    #[error("Tab limit in group exceeded")]
    TabLimitExceeded,
}