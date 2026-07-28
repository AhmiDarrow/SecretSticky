use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("vault is locked")]
    Locked,
    #[error("vault already unlocked")]
    AlreadyUnlocked,
    #[error("invalid master password")]
    BadPassword,
    #[error("vault not initialized")]
    NotInitialized,
    #[error("vault already exists")]
    AlreadyInitialized,
    #[error("note not found")]
    NoteNotFound,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("crypto: {0}")]
    Crypto(String),
    #[error("serialize: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
