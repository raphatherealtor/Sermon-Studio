use thiserror::Error;

/// Unified error type for the Sermon Studio core engine.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("yaml frontmatter error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid reference '{0}': {1}")]
    BadReference(String, String),

    #[error("missing required frontmatter field: {0}")]
    MissingField(String),

    #[error("vault path does not exist: {0}")]
    VaultMissing(String),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;
