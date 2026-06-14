pub mod mysql;
pub mod postgres;
pub mod redis;
pub mod sqlite;

use crate::db::error::DbError;
use crate::db::traits::SqlClient;

pub enum ConnectorType {
    Postgres,
    Sqlite,
    Mysql,
    Redis,
}

impl ConnectorType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "postgres" | "postgresql" => Some(Self::Postgres),
            "sqlite"                  => Some(Self::Sqlite),
            "mysql" | "mariadb"       => Some(Self::Mysql),
            "redis"                   => Some(Self::Redis),
            _                         => None,
        }
    }
}

pub fn create_sql_connector(t: &ConnectorType) -> Result<Box<dyn SqlClient>, DbError> {
    match t {
        ConnectorType::Postgres => Ok(Box::new(postgres::PostgresConnector::new())),
        ConnectorType::Sqlite   => Ok(Box::new(sqlite::SqliteConnector::new())),
        ConnectorType::Mysql    => Ok(Box::new(mysql::MySqlConnector::new())),
        ConnectorType::Redis    => Err(DbError::Unsupported(
            "Redis uses KvClient, not SqlClient".into(),
        )),
    }
}
