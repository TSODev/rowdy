use async_trait::async_trait;
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::DbQueryResult;

pub struct SqliteConnector {
    // pool: Option<sqlx::SqlitePool>,
}

impl SqliteConnector {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl SqlClient for SqliteConnector {
    async fn connect(&mut self, _url: &str) -> Result<(), DbError> {
        todo!()
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        todo!()
    }

    async fn execute(&self, _query: &str) -> Result<u64, DbError> {
        todo!()
    }

    async fn fetch_all(&self, _query: &str) -> Result<DbQueryResult, DbError> {
        todo!()
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        todo!()
    }
}
