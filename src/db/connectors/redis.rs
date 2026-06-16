use async_trait::async_trait;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::db::error::DbError;
use crate::db::traits::KvClient;
use crate::db::types::KvKeyDetail;

// Commands on MultiplexedConnection require &mut self, so we wrap in a Mutex
// to satisfy the &self signature imposed by KvClient.
pub struct RedisConnector {
    conn: Arc<Mutex<Option<redis::aio::MultiplexedConnection>>>,
}

impl RedisConnector {
    pub fn new() -> Self {
        Self {
            conn: Arc::new(Mutex::new(None)),
        }
    }

    async fn with_conn<F, T>(&self, f: F) -> Result<T, DbError>
    where
        F: AsyncFnOnce(&mut redis::aio::MultiplexedConnection) -> Result<T, DbError>,
    {
        let mut guard = self.conn.lock().await;
        let conn = guard.as_mut().ok_or(DbError::NotConnected)?;
        f(conn).await
    }
}

#[async_trait]
impl KvClient for RedisConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let client = redis::Client::open(url)
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        let conn = client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        *self.conn.lock().await = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        *self.conn.lock().await = None;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<String>, DbError> {
        self.with_conn(async |conn| {
            conn.get::<_, Option<String>>(key)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))
        })
        .await
    }

    async fn set(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.with_conn(async |conn| {
            conn.set::<_, _, ()>(key, value)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))
        })
        .await
    }

    async fn del(&self, key: &str) -> Result<bool, DbError> {
        self.with_conn(async |conn| {
            let count: i64 = conn
                .del(key)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;
            Ok(count > 0)
        })
        .await
    }

    async fn keys(&self, pattern: &str) -> Result<Vec<String>, DbError> {
        self.with_conn(async |conn| {
            conn.keys::<_, Vec<String>>(pattern)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))
        })
        .await
    }

    async fn get_key_detail(&self, key: &str) -> Result<KvKeyDetail, DbError> {
        self.with_conn(async |conn| {
            let key_type: String = redis::cmd("TYPE")
                .arg(key)
                .query_async(conn)
                .await
                .map_err(|e| DbError::QueryFailed(e.to_string()))?;

            match key_type.as_str() {
                "string" => {
                    let value: String = conn.get(key).await
                        .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                    Ok(KvKeyDetail::String(value))
                }
                "hash" => {
                    let map: HashMap<String, String> = conn.hgetall(key).await
                        .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                    let mut pairs: Vec<(String, String)> = map.into_iter().collect();
                    pairs.sort_by(|a, b| a.0.cmp(&b.0));
                    Ok(KvKeyDetail::Hash(pairs))
                }
                "list" => {
                    let items: Vec<String> = conn.lrange(key, 0, -1).await
                        .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                    Ok(KvKeyDetail::List(items))
                }
                "set" => {
                    let mut members: Vec<String> = conn.smembers(key).await
                        .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                    members.sort();
                    Ok(KvKeyDetail::Set(members))
                }
                "zset" => {
                    let pairs: Vec<(String, f64)> = conn.zrange_withscores(key, 0isize, -1isize).await
                        .map_err(|e| DbError::QueryFailed(e.to_string()))?;
                    Ok(KvKeyDetail::ZSet(pairs))
                }
                other => Err(DbError::QueryFailed(format!("Unknown key type: {other}"))),
            }
        })
        .await
    }

    async fn ttl(&self, key: &str) -> Result<i64, DbError> {
        self.with_conn(async |conn| {
            conn.ttl::<_, i64>(key).await
                .map_err(|e| DbError::QueryFailed(e.to_string()))
        })
        .await
    }
}
