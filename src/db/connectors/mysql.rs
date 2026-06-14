use async_trait::async_trait;
use sqlx::{
    mysql::{MySqlPool, MySqlRow},
    Column as SqlxColumn, Row as SqlxRow, TypeInfo, ValueRef,
};
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, Value};

pub struct MySqlConnector {
    pool: Option<MySqlPool>,
}

impl MySqlConnector {
    pub fn new() -> Self {
        Self { pool: None }
    }

    fn pool(&self) -> Result<&MySqlPool, DbError> {
        self.pool.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for MySqlConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let pool = MySqlPool::connect(url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        self.pool = Some(pool);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        if let Some(pool) = self.pool.take() {
            pool.close().await;
        }
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<u64, DbError> {
        let result = sqlx::query(query)
            .execute(self.pool()?)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError> {
        let rows: Vec<MySqlRow> = sqlx::query(query)
            .fetch_all(self.pool()?)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        if rows.is_empty() {
            return Ok(DbQueryResult {
                columns: vec![],
                rows: vec![],
                rows_affected: 0,
            });
        }

        let columns: Vec<Column> = rows[0]
            .columns()
            .iter()
            .map(|c| Column {
                name: c.name().to_string(),
                type_name: c.type_info().name().to_string(),
            })
            .collect();

        let mapped_rows: Vec<Row> = rows
            .iter()
            .map(|r| Row {
                values: (0..r.len()).map(|i| mysql_value(r, i)).collect(),
            })
            .collect();

        let count = mapped_rows.len() as u64;
        Ok(DbQueryResult {
            columns,
            rows: mapped_rows,
            rows_affected: count,
        })
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        let rows: Vec<MySqlRow> = sqlx::query(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = DATABASE() AND table_type = 'BASE TABLE' \
             ORDER BY table_name",
        )
        .fetch_all(self.pool()?)
        .await
        .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| r.try_get::<String, _>(0).unwrap_or_default())
            .collect())
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let pool = self.pool()?;
        let safe = table.replace('\'', "").replace('\\', "");
        let query = format!(
            "SELECT c.COLUMN_NAME, c.DATA_TYPE, c.IS_NULLABLE, c.COLUMN_KEY, \
             kcu.REFERENCED_TABLE_NAME AS FK_TABLE, kcu.REFERENCED_COLUMN_NAME AS FK_COLUMN \
             FROM information_schema.COLUMNS c \
             LEFT JOIN information_schema.KEY_COLUMN_USAGE kcu \
                 ON kcu.TABLE_SCHEMA = c.TABLE_SCHEMA \
                 AND kcu.TABLE_NAME = c.TABLE_NAME \
                 AND kcu.COLUMN_NAME = c.COLUMN_NAME \
                 AND kcu.REFERENCED_TABLE_NAME IS NOT NULL \
             WHERE c.TABLE_SCHEMA = DATABASE() AND c.TABLE_NAME = '{safe}' \
             ORDER BY c.ORDINAL_POSITION"
        );

        let rows: Vec<MySqlRow> = sqlx::query(&query)
            .fetch_all(pool)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let mut schema = vec![];
        for row in &rows {
            let name: String = row.try_get("COLUMN_NAME").unwrap_or_default();
            let type_name: String = row.try_get("DATA_TYPE").unwrap_or_default();
            let is_nullable: String = row.try_get("IS_NULLABLE").unwrap_or_else(|_| "YES".into());
            let column_key: String = row.try_get("COLUMN_KEY").unwrap_or_default();
            let fk_table: Option<String> = row.try_get::<Option<String>, _>("FK_TABLE").unwrap_or(None);
            let fk_col: Option<String> = row.try_get::<Option<String>, _>("FK_COLUMN").unwrap_or(None);
            let fk = match (fk_table, fk_col) {
                (Some(t), Some(c)) if !t.is_empty() => Some(ForeignKey { table: t, column: c }),
                _ => None,
            };
            schema.push(ColumnSchema {
                name,
                type_name,
                is_pk: column_key == "PRI",
                is_nullable: is_nullable == "YES",
                fk,
            });
        }
        Ok(schema)
    }
}

fn mysql_value(row: &MySqlRow, index: usize) -> Value {
    let raw = row.try_get_raw(index).unwrap();
    if raw.is_null() {
        return Value::Null;
    }
    match raw.type_info().name() {
        "BOOLEAN" | "TINYINT(1)"             => row.try_get::<bool, _>(index).map(Value::Bool).unwrap_or(Value::Null),
        // Signed integers — each width needs its own Rust type
        "TINYINT"                            => row.try_get::<i8, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "SMALLINT"                           => row.try_get::<i16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "INT" | "MEDIUMINT"                  => row.try_get::<i32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "BIGINT"                             => row.try_get::<i64, _>(index).map(Value::Int).unwrap_or(Value::Null),
        // Unsigned variants
        "TINYINT UNSIGNED"                   => row.try_get::<u8, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "SMALLINT UNSIGNED"                  => row.try_get::<u16, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "INT UNSIGNED" | "MEDIUMINT UNSIGNED"=> row.try_get::<u32, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "BIGINT UNSIGNED"                    => row.try_get::<u64, _>(index).map(|v| Value::Int(v as i64)).unwrap_or(Value::Null),
        "FLOAT"                              => row.try_get::<f32, _>(index).map(|v| Value::Float(v as f64)).unwrap_or(Value::Null),
        "DOUBLE"                             => row.try_get::<f64, _>(index).map(Value::Float).unwrap_or(Value::Null),
        // DECIMAL: MySQL wire sends it as text — parse to f64, keep as Text if not parseable
        "DECIMAL"                            => row.try_get::<String, _>(index).ok()
            .map(|s| s.parse::<f64>().map(Value::Float).unwrap_or_else(|_| Value::Text(s)))
            .unwrap_or(Value::Null),
        "BLOB" | "TINYBLOB" | "MEDIUMBLOB" | "LONGBLOB" | "BINARY" | "VARBINARY"
                                             => row.try_get::<Vec<u8>, _>(index).map(Value::Bytes).unwrap_or(Value::Null),
        _ => row.try_get::<String, _>(index).map(Value::Text).unwrap_or(Value::Null),
    }
}
