use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Query failed: {0}")]
    QueryFailed(String),
    #[error("Not connected")]
    NotConnected,
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
}
