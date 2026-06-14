use async_trait::async_trait;
use crate::db::error::DbError;
use crate::db::types::DbQueryResult;

#[async_trait]
pub trait SqlClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn execute(&self, query: &str) -> Result<u64, DbError>;
    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError>;
    async fn get_tables(&self) -> Result<Vec<String>, DbError>;
}
