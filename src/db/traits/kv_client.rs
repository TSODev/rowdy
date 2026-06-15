use async_trait::async_trait;
use crate::db::error::DbError;

#[async_trait]
pub trait KvClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    #[allow(dead_code)]
    async fn disconnect(&mut self) -> Result<(), DbError>;
    #[allow(dead_code)]
    async fn get(&self, key: &str) -> Result<Option<String>, DbError>;
    #[allow(dead_code)]
    async fn set(&self, key: &str, value: &str) -> Result<(), DbError>;
    #[allow(dead_code)]
    async fn del(&self, key: &str) -> Result<bool, DbError>;
    async fn keys(&self, pattern: &str) -> Result<Vec<String>, DbError>;
}
