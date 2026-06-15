pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;
pub mod turso;

use crate::db::error::DbError;
use crate::db::traits::{KvClient, SqlClient};

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
