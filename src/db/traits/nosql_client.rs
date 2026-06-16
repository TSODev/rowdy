use async_trait::async_trait;
use crate::db::error::DbError;
use crate::db::types::{DbQueryResult, TableObject};

#[allow(dead_code)]
#[async_trait]
pub trait NoSqlClient: Send + Sync {
    async fn connect(&mut self, url: &str) -> Result<(), DbError>;
    async fn disconnect(&mut self) -> Result<(), DbError>;
    async fn list_collections(&self) -> Result<Vec<TableObject>, DbError>;
    async fn find(&self, collection: &str, filter: &str, limit: u64, offset: u64) -> Result<DbQueryResult, DbError>;
    async fn aggregate(&self, collection: &str, pipeline: &str) -> Result<DbQueryResult, DbError>;
    async fn count(&self, collection: &str, filter: &str) -> Result<u64, DbError>;
}
