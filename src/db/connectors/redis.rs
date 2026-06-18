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

#[cfg(test)]
mod tests {
    use super::RedisConnector;
    use crate::db::{error::DbError, traits::KvClient, types::KvKeyDetail};
    use redis::AsyncCommands;
    use std::sync::atomic::{AtomicU32, Ordering};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn redis_url() -> Option<String> {
        std::env::var("REDIS_URL").ok()
    }

    /// Each test gets a unique key prefix to avoid cross-test conflicts.
    fn prefix() -> String {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        format!("_rowdy_redis_test_{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    async fn connected(url: &str) -> RedisConnector {
        let mut c = RedisConnector::new();
        c.connect(url).await.expect("connect failed");
        println!("  [redis] connected to {url}");
        c
    }

    /// Opens a raw multiplexed connection for test setup (Hash/List/Set/ZSet seeding).
    async fn raw_conn(url: &str) -> redis::aio::MultiplexedConnection {
        redis::Client::open(url)
            .expect("invalid Redis URL")
            .get_multiplexed_tokio_connection()
            .await
            .expect("raw connect failed")
    }

    // ── connection ───────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_connect() {
        println!("\n[redis] test_connect");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let mut c = RedisConnector::new();
        assert!(c.connect(&url).await.is_ok(), "connect should succeed");
        println!("  ✓ connection OK");
    }

    #[tokio::test]
    async fn test_not_connected_returns_error() {
        println!("\n[redis] test_not_connected_returns_error");
        let c = RedisConnector::new();
        let e = c.get("any_key").await.unwrap_err();
        assert!(matches!(e, DbError::NotConnected));
        println!("  ✓ get before connect → NotConnected");
        let e2 = c.keys("*").await.unwrap_err();
        assert!(matches!(e2, DbError::NotConnected));
        println!("  ✓ keys before connect → NotConnected");
    }

    #[tokio::test]
    async fn test_disconnect() {
        println!("\n[redis] test_disconnect");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let mut c = connected(&url).await;
        assert!(c.disconnect().await.is_ok());
        // After disconnect, operations should fail
        let e = c.get("x").await.unwrap_err();
        assert!(matches!(e, DbError::NotConnected));
        println!("  ✓ disconnect OK, subsequent get → NotConnected");
    }

    // ── get / set / del ──────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_set_and_get() {
        println!("\n[redis] test_set_and_get");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_str", prefix());

        c.set(&key, "hello rowdy").await.expect("set failed");
        let val = c.get(&key).await.expect("get failed");
        println!("  get({key}) = {:?}", val);
        assert_eq!(val, Some("hello rowdy".to_string()));

        c.del(&key).await.ok();
        println!("  ✓ set + get roundtrip OK");
    }

    #[tokio::test]
    async fn test_get_nonexistent_key() {
        println!("\n[redis] test_get_nonexistent_key");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_noexist", prefix());
        let val = c.get(&key).await.expect("get failed");
        assert_eq!(val, None, "nonexistent key should return None");
        println!("  ✓ get(nonexistent) → None");
    }

    #[tokio::test]
    async fn test_del_existing_key() {
        println!("\n[redis] test_del_existing_key");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_del", prefix());

        c.set(&key, "value").await.unwrap();
        let deleted = c.del(&key).await.expect("del failed");
        println!("  del({key}) = {deleted}");
        assert!(deleted, "del should return true for existing key");
        assert_eq!(c.get(&key).await.unwrap(), None, "key should be gone");
        println!("  ✓ del existing key → true, key gone");
    }

    #[tokio::test]
    async fn test_del_nonexistent_key() {
        println!("\n[redis] test_del_nonexistent_key");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_noexist_del", prefix());
        let deleted = c.del(&key).await.expect("del failed");
        assert!(!deleted, "del should return false for nonexistent key");
        println!("  ✓ del nonexistent key → false");
    }

    #[tokio::test]
    async fn test_keys_pattern() {
        println!("\n[redis] test_keys_pattern");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let pfx = prefix();
        let k1 = format!("{pfx}_a");
        let k2 = format!("{pfx}_b");
        let k3 = format!("{pfx}_c");

        c.set(&k1, "1").await.unwrap();
        c.set(&k2, "2").await.unwrap();
        c.set(&k3, "3").await.unwrap();

        let mut found = c.keys(&format!("{pfx}_*")).await.expect("keys failed");
        found.sort();
        println!("  keys({pfx}_*) = {:?}", found);
        assert_eq!(found.len(), 3);
        assert!(found.contains(&k1) && found.contains(&k2) && found.contains(&k3));

        for k in [&k1, &k2, &k3] { c.del(k).await.ok(); }
        println!("  ✓ keys pattern matched 3 keys");
    }

    // ── get_key_detail — all 5 Redis types ───────────────────────────────────────

    #[tokio::test]
    async fn test_key_detail_string() {
        println!("\n[redis] test_key_detail_string");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_kd_string", prefix());

        c.set(&key, "hello").await.unwrap();
        let detail = c.get_key_detail(&key).await.expect("get_key_detail failed");
        println!("  detail: {:?}", detail);
        assert!(matches!(detail, KvKeyDetail::String(ref s) if s == "hello"));

        c.del(&key).await.ok();
        println!("  ✓ String key → KvKeyDetail::String");
    }

    #[tokio::test]
    async fn test_key_detail_hash() {
        println!("\n[redis] test_key_detail_hash");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_kd_hash", prefix());
        let mut raw = raw_conn(&url).await;

        // Set up hash via raw connection
        let _: () = raw.hset_multiple(&key, &[("city", "Paris"), ("zip", "75001"), ("country", "FR")]).await.unwrap();

        let detail = c.get_key_detail(&key).await.expect("get_key_detail failed");
        println!("  detail: {:?}", detail);
        let KvKeyDetail::Hash(pairs) = detail else { panic!("expected Hash, got {:?}", detail) };
        // Connector sorts by field name
        assert_eq!(pairs[0].0, "city");
        assert_eq!(pairs[0].1, "Paris");
        assert_eq!(pairs[1].0, "country");
        assert_eq!(pairs[2].0, "zip");

        let _: () = raw.del(&key).await.unwrap();
        println!("  ✓ Hash key → KvKeyDetail::Hash (sorted by field)");
    }

    #[tokio::test]
    async fn test_key_detail_list() {
        println!("\n[redis] test_key_detail_list");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_kd_list", prefix());
        let mut raw = raw_conn(&url).await;

        let _: () = raw.rpush(&key, &["first", "second", "third"]).await.unwrap();

        let detail = c.get_key_detail(&key).await.expect("get_key_detail failed");
        println!("  detail: {:?}", detail);
        let KvKeyDetail::List(items) = detail else { panic!("expected List") };
        assert_eq!(items, vec!["first", "second", "third"]);

        let _: () = raw.del(&key).await.unwrap();
        println!("  ✓ List key → KvKeyDetail::List (insertion order)");
    }

    #[tokio::test]
    async fn test_key_detail_set() {
        println!("\n[redis] test_key_detail_set");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_kd_set", prefix());
        let mut raw = raw_conn(&url).await;

        let _: () = raw.sadd(&key, &["banana", "apple", "cherry"]).await.unwrap();

        let detail = c.get_key_detail(&key).await.expect("get_key_detail failed");
        println!("  detail: {:?}", detail);
        let KvKeyDetail::Set(members) = detail else { panic!("expected Set") };
        // Connector sorts alphabetically
        assert_eq!(members, vec!["apple", "banana", "cherry"]);

        let _: () = raw.del(&key).await.unwrap();
        println!("  ✓ Set key → KvKeyDetail::Set (sorted alphabetically)");
    }

    #[tokio::test]
    async fn test_key_detail_zset() {
        println!("\n[redis] test_key_detail_zset");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_kd_zset", prefix());
        let mut raw = raw_conn(&url).await;

        let _: () = raw.zadd_multiple(&key, &[(1.0f64, "bronze"), (2.0f64, "silver"), (3.0f64, "gold")]).await.unwrap();

        let detail = c.get_key_detail(&key).await.expect("get_key_detail failed");
        println!("  detail: {:?}", detail);
        let KvKeyDetail::ZSet(pairs) = detail else { panic!("expected ZSet") };
        assert_eq!(pairs[0], ("bronze".to_string(), 1.0));
        assert_eq!(pairs[1], ("silver".to_string(), 2.0));
        assert_eq!(pairs[2], ("gold".to_string(),   3.0));

        let _: () = raw.del(&key).await.unwrap();
        println!("  ✓ ZSet key → KvKeyDetail::ZSet (ordered by score)");
    }

    // ── ttl ──────────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_ttl_no_expiry() {
        println!("\n[redis] test_ttl_no_expiry");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_ttl_none", prefix());

        c.set(&key, "persistent").await.unwrap();
        let ttl = c.ttl(&key).await.expect("ttl failed");
        println!("  ttl({key}) = {ttl}");
        assert_eq!(ttl, -1, "persistent key should have TTL = -1");

        c.del(&key).await.ok();
        println!("  ✓ persistent key → TTL = -1");
    }

    #[tokio::test]
    async fn test_ttl_with_expiry() {
        println!("\n[redis] test_ttl_with_expiry");
        let Some(url) = redis_url() else {
            println!("  REDIS_URL not set — skipped");
            return;
        };
        let c = connected(&url).await;
        let key = format!("{}_ttl_expiry", prefix());
        let mut raw = raw_conn(&url).await;

        let _: () = raw.set_ex(&key, "temporary", 300u64).await.unwrap();
        let ttl = c.ttl(&key).await.expect("ttl failed");
        println!("  ttl({key}) = {ttl}s");
        assert!(ttl > 0 && ttl <= 300, "TTL should be between 1 and 300, got {ttl}");

        let _: () = raw.del(&key).await.unwrap();
        println!("  ✓ key with 300s expiry → TTL > 0");
    }
}
