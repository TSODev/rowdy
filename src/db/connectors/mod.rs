pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;
pub mod turso;
#[cfg(feature = "mongodb")]
pub mod mongodb;

use crate::db::error::DbError;
use crate::db::traits::{KvClient, NoSqlClient, SqlClient};

/// Create and connect a SQL connector for the given driver name and URL.
pub async fn connect_sql(db_type: &str, url: &str) -> Result<Box<dyn SqlClient>, DbError> {
    match db_type.to_lowercase().as_str() {
        "postgres" | "postgresql" => {
            let mut c = postgres::PostgresConnector::new();
            c.connect(url).await?;
            Ok(Box::new(c))
        }
        "sqlite" => {
            let mut c = sqlite::SqliteConnector::new();
            c.connect(url).await?;
            Ok(Box::new(c))
        }
        "mysql" | "mariadb" => {
            let mut c = mysql::MySqlConnector::new();
            c.connect(url).await?;
            Ok(Box::new(c))
        }
        "turso" | "libsql" => {
            let mut c = turso::TursoClient::new();
            c.connect(url).await?;
            Ok(Box::new(c))
        }
        other => Err(DbError::Unsupported(format!("Unknown SQL driver: {other}"))),
    }
}

/// Create and connect a key-value connector for the given driver name and URL.
pub async fn connect_kv(db_type: &str, url: &str) -> Result<Box<dyn KvClient>, DbError> {
    match db_type.to_lowercase().as_str() {
        "redis" => {
            let mut c = redis::RedisConnector::new();
            c.connect(url).await?;
            Ok(Box::new(c))
        }
        other => Err(DbError::Unsupported(format!("Unknown KV driver: {other}"))),
    }
}

/// Create and connect a NoSQL connector for the given driver name and URL.
#[allow(dead_code, unused_variables)]
pub async fn connect_nosql(db_type: &str, url: &str) -> Result<Box<dyn NoSqlClient>, DbError> {
    match db_type.to_lowercase().as_str() {
        "mongodb" => {
            #[cfg(feature = "mongodb")]
            {
                let mut c = mongodb::MongoDbConnector::new();
                c.connect(url).await?;
                return Ok(Box::new(c));
            }
            #[cfg(not(feature = "mongodb"))]
            Err(DbError::Unsupported(
                "MongoDB support not compiled in — reinstall with --features mongodb".to_string(),
            ))
        }
        other => Err(DbError::Unsupported(format!("Unknown NoSQL driver: {other}"))),
    }
}
