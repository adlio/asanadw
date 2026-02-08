use std::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Asana API error: {0}")]
    Api(#[from] asanaclient::Error),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Migration error: {0}")]
    Migration(String),

    #[error("Sync error for {entity_key}: {message}")]
    Sync { entity_key: String, message: String },

    #[error("Invalid URL: {0}")]
    UrlParse(String),

    #[error("Invalid identifier: {0}")]
    InvalidIdentifier(String),

    #[error("Invalid period format: {0}")]
    PeriodParse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("Entity not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Database(e.to_string())
    }
}

impl From<rusqlite_migration::Error> for Error {
    fn from(e: rusqlite_migration::Error) -> Self {
        Error::Migration(e.to_string())
    }
}

impl<E: fmt::Display> From<tokio_rusqlite::Error<E>> for Error {
    fn from(e: tokio_rusqlite::Error<E>) -> Self {
        Error::Database(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
