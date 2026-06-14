use async_trait::async_trait;
use crate::db::error::DbError;
use crate::db::traits::KvClient;

pub struct RedisConnector {
    // client: Option<redis::aio::Connection>,
}

impl RedisConnector {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl KvClient for RedisConnector {
    async fn connect(&mut self, _url: &str) -> Result<(), DbError> {
        todo!()
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        todo!()
    }

    async fn get(&self, _key: &str) -> Result<Option<String>, DbError> {
        todo!()
    }

    async fn set(&self, _key: &str, _value: &str) -> Result<(), DbError> {
        todo!()
    }

    async fn del(&self, _key: &str) -> Result<bool, DbError> {
        todo!()
    }

    async fn keys(&self, _pattern: &str) -> Result<Vec<String>, DbError> {
        todo!()
    }
}
