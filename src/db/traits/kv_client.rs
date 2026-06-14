use async_trait::async_trait;
use crate::db::error::DbError;

#[async_trait]
pub trait KvClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn get(&self, key: &str) -> Result<Option<String>, DbError>;
    async fn set(&self, key: &str, value: &str) -> Result<(), DbError>;
    async fn del(&self, key: &str) -> Result<bool, DbError>;
    async fn keys(&self, pattern: &str) -> Result<Vec<String>, DbError>;
}
