use thiserror::Error;

#[derive(Debug, Error)]
pub enum EasyMcError {
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("{0}")]
    Message(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("http: {0}")]
    Http(String),
    #[error("missing: {0}")]
    Missing(String),
}

pub type Result<T> = std::result::Result<T, EasyMcError>;

impl EasyMcError {
    pub fn msg(s: impl Into<String>) -> Self {
        Self::Message(s.into())
    }
}
