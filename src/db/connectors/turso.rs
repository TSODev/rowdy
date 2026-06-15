use async_trait::async_trait;
use std::collections::HashMap;
use crate::db::error::DbError;
use crate::db::traits::SqlClient;
use crate::db::types::{Column, ColumnSchema, DbQueryResult, ForeignKey, Row, TableKind, TableObject, Value};

pub struct TursoClient {
    db:   Option<libsql::Database>,
    conn: Option<libsql::Connection>,
}

impl TursoClient {
    pub fn new() -> Self {
        Self { db: None, conn: None }
    }

    fn conn(&self) -> Result<&libsql::Connection, DbError> {
        self.conn.as_ref().ok_or(DbError::NotConnected)
    }
}

#[async_trait]
impl SqlClient for TursoClient {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let (base_url, token) = parse_url(url)?;
        let db = libsql::Builder::new_remote(base_url, token)
            .build()
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        let conn = db.connect()
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;
        self.db   = Some(db);
        self.conn = Some(conn);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        self.conn = None;
        self.db   = None;
        Ok(())
    }

    async fn execute(&self, query: &str) -> Result<u64, DbError> {
        self.conn()?
            .execute(query, ())
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))
    }

    async fn fetch_all(&self, query: &str) -> Result<DbQueryResult, DbError> {
        let mut rows = self.conn()?
            .query(query, ())
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let col_count = rows.column_count(); // i32

        let columns: Vec<Column> = (0..col_count)
            .map(|i| Column {
                name:      rows.column_name(i).unwrap_or("").to_string(),
                type_name: String::new(),
            })
            .collect();

        let mut result_rows: Vec<Row> = vec![];
        while let Some(row) = rows.next().await.map_err(|e| DbError::QueryFailed(e.to_string()))? {
            let values = (0..col_count)
                .map(|i| to_value(row.get_value(i).unwrap_or(libsql::Value::Null)))
                .collect();
            result_rows.push(Row { values });
        }

        let count = result_rows.len() as u64;
        Ok(DbQueryResult { columns, rows: result_rows, rows_affected: count })
    }

    async fn get_tables(&self) -> Result<Vec<String>, DbError> {
        let result = self.fetch_all(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        ).await?;
        Ok(result.rows.iter()
            .filter_map(|r| {
                if let Some(Value::Text(s)) = r.values.first() { Some(s.clone()) } else { None }
            })
            .collect())
    }

    async fn get_table_objects(&self) -> Result<Vec<TableObject>, DbError> {
        let result = self.fetch_all(
            "SELECT name, type FROM sqlite_master \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' ORDER BY name"
        ).await?;
        Ok(result.rows.iter().filter_map(|r| {
            let name = if let Some(Value::Text(s)) = r.values.first() { s.clone() } else { return None; };
            let type_str = r.values.get(1).and_then(|v| if let Value::Text(s) = v { Some(s.as_str()) } else { None }).unwrap_or("table");
            let kind = if type_str == "view" { TableKind::View } else { TableKind::Table };
            Some(TableObject { name, kind })
        }).collect())
    }

    async fn get_schema(&self, table: &str) -> Result<Vec<ColumnSchema>, DbError> {
        let safe = table.replace('"', "");

        let info    = self.fetch_all(&format!("PRAGMA table_info(\"{}\")", safe)).await?;
        let fk_list = self.fetch_all(&format!("PRAGMA foreign_key_list(\"{}\")", safe)).await?;

        // PRAGMA foreign_key_list: id(0) seq(1) table(2) from(3) to(4) …
        let mut fk_map: HashMap<String, ForeignKey> = HashMap::new();
        for row in &fk_list.rows {
            if let (Some(Value::Text(from)), Some(Value::Text(tbl)), Some(Value::Text(to))) =
                (row.values.get(3), row.values.get(2), row.values.get(4))
            {
                fk_map.insert(from.clone(), ForeignKey { table: tbl.clone(), column: to.clone() });
            }
        }

        // PRAGMA table_info: cid(0) name(1) type(2) notnull(3) dflt_value(4) pk(5)
        let schema = info.rows.iter()
            .filter_map(|row| {
                let name = match row.values.get(1)? {
                    Value::Text(s) => s.clone(),
                    _ => return None,
                };
                let type_name = match row.values.get(2)? {
                    Value::Text(s) => s.clone(),
                    _ => String::new(),
                };
                let notnull   = matches!(row.values.get(3), Some(Value::Int(1)));
                let pk        = matches!(row.values.get(5), Some(Value::Int(n)) if *n > 0);
                let fk        = fk_map.get(&name).cloned();
                Some(ColumnSchema { name, type_name, is_pk: pk, is_nullable: !notnull, fk })
            })
            .collect();

        Ok(schema)
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn to_value(v: libsql::Value) -> Value {
    match v {
        libsql::Value::Null        => Value::Null,
        libsql::Value::Integer(n)  => Value::Int(n),
        libsql::Value::Real(f)     => Value::Float(f),
        libsql::Value::Text(s)     => Value::Text(s),
        libsql::Value::Blob(b)     => Value::Bytes(b),
    }
}

/// Parse `libsql://host?authToken=TOKEN` into `(base_url, token)`.
fn parse_url(url: &str) -> Result<(String, String), DbError> {
    if let Some(pos) = url.find("?authToken=") {
        let base  = url[..pos].to_string();
        let token = url[pos + "?authToken=".len()..].to_string();
        if base.is_empty() || token.is_empty() {
            return Err(DbError::ConnectionFailed(
                "Invalid Turso URL: base URL or authToken is empty".into(),
            ));
        }
        Ok((base, token))
    } else {
        Err(DbError::ConnectionFailed(
            "Invalid Turso URL. Expected format: libsql://host?authToken=TOKEN".into(),
        ))
    }
}
