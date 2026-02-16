use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] crate::db::DbError),
    #[error("clipboard error: {0}")]
    Clipboard(#[from] crate::clipboard::ClipboardError),
    #[error("not found")]
    NotFound,
    #[error("internal error: {0}")]
    Internal(String),
}

pub type AppResult<T> = Result<T, AppError>;
