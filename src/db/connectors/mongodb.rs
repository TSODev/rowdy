#![cfg(feature = "mongodb")]

use async_trait::async_trait;
use futures::stream::TryStreamExt;
use mongodb::{
    bson::{self, Bson, Document},
    options::{ClientOptions, FindOptions},
    Client,
};

use crate::db::error::DbError;
use crate::db::traits::NoSqlClient;
use crate::db::types::{Column, DbQueryResult, Row, TableKind, TableObject, Value};

pub struct MongoDbConnector {
    client: Option<Client>,
    db_name: Option<String>,
}

impl MongoDbConnector {
    pub fn new() -> Self {
        Self { client: None, db_name: None }
    }

    fn get_db(&self) -> Result<mongodb::Database, DbError> {
        let client = self.client.as_ref().ok_or(DbError::NotConnected)?;
        let name = self.db_name.as_deref().ok_or(DbError::NotConnected)?;
        Ok(client.database(name))
    }
}

#[async_trait]
impl NoSqlClient for MongoDbConnector {
    async fn connect(&mut self, url: &str) -> Result<(), DbError> {
        let opts = ClientOptions::parse(url)
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // Extract database from URI path or reject
        let db_name = opts.default_database.clone().ok_or_else(|| {
            DbError::ConnectionFailed(
                "No database specified in URL — use mongodb://host:27017/dbname".to_string(),
            )
        })?;

        let client =
            Client::with_options(opts).map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        // Verify connectivity with a ping
        client
            .database(&db_name)
            .run_command(bson::doc! { "ping": 1 })
            .await
            .map_err(|e| DbError::ConnectionFailed(e.to_string()))?;

        self.client = Some(client);
        self.db_name = Some(db_name);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), DbError> {
        self.client = None;
        self.db_name = None;
        Ok(())
    }

    async fn list_collections(&self) -> Result<Vec<TableObject>, DbError> {
        let db = self.get_db()?;
        let names = db
            .list_collection_names()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;
        let mut objects: Vec<TableObject> = names
            .into_iter()
            .map(|name| TableObject { name, kind: TableKind::Table })
            .collect();
        objects.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(objects)
    }

    async fn find(
        &self,
        collection: &str,
        filter: &str,
        limit: u64,
        offset: u64,
    ) -> Result<DbQueryResult, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);

        let filter_doc = parse_filter(filter)?;

        let opts = FindOptions::builder()
            .limit(limit as i64)
            .skip(offset)
            .build();

        let cursor = coll
            .find(filter_doc)
            .with_options(opts)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(docs_to_result(docs))
    }

    async fn aggregate(
        &self,
        collection: &str,
        pipeline: &str,
    ) -> Result<DbQueryResult, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);

        let pipeline_docs = parse_pipeline(pipeline)?;

        let cursor = coll
            .aggregate(pipeline_docs)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        let docs: Vec<Document> = cursor
            .try_collect()
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))?;

        Ok(docs_to_result(docs))
    }

    async fn count(&self, collection: &str, filter: &str) -> Result<u64, DbError> {
        let db = self.get_db()?;
        let coll = db.collection::<Document>(collection);
        let filter_doc = parse_filter(filter)?;
        coll.count_documents(filter_doc)
            .await
            .map_err(|e| DbError::QueryFailed(e.to_string()))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn parse_filter(filter: &str) -> Result<Document, DbError> {
    let s = filter.trim();
    if s.is_empty() || s == "{}" {
        return Ok(bson::doc! {});
    }
    let json: serde_json::Value = serde_json::from_str(s)
        .map_err(|e| DbError::QueryFailed(format!("Invalid filter JSON: {e}")))?;
    let bson_val =
        bson::to_bson(&json).map_err(|e| DbError::QueryFailed(format!("BSON error: {e}")))?;
    bson_val
        .as_document()
        .cloned()
        .ok_or_else(|| DbError::QueryFailed("Filter must be a JSON object {{ }}".to_string()))
}

fn parse_pipeline(pipeline: &str) -> Result<Vec<Document>, DbError> {
    let s = pipeline.trim();
    let json: Vec<serde_json::Value> = serde_json::from_str(s)
        .map_err(|e| DbError::QueryFailed(format!("Invalid pipeline JSON: {e}")))?;
    json.iter()
        .map(|v| {
            let bson_val = bson::to_bson(v)
                .map_err(|e| DbError::QueryFailed(format!("BSON error: {e}")))?;
            bson_val
                .as_document()
                .cloned()
                .ok_or_else(|| DbError::QueryFailed("Pipeline stage must be a JSON object".to_string()))
        })
        .collect()
}

/// Convert a batch of BSON documents into a DbQueryResult.
/// Columns are built from the union of all top-level keys, `_id` first.
fn docs_to_result(docs: Vec<Document>) -> DbQueryResult {
    let mut col_names: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for doc in &docs {
        for key in doc.keys() {
            if seen.insert(key.clone()) {
                // Keep _id at front
                if key == "_id" {
                    col_names.insert(0, key.clone());
                } else {
                    col_names.push(key.clone());
                }
            }
        }
    }

    let columns: Vec<Column> = col_names
        .iter()
        .map(|name| Column { name: name.clone(), type_name: "bson".to_string() })
        .collect();

    let rows: Vec<Row> = docs
        .iter()
        .map(|doc| {
            let values = col_names
                .iter()
                .map(|col| doc.get(col).map(bson_to_value).unwrap_or(Value::Null))
                .collect();
            Row { values }
        })
        .collect();

    DbQueryResult { columns, rows, rows_affected: 0 }
}

fn bson_to_value(bson: &Bson) -> Value {
    match bson {
        Bson::Null | Bson::Undefined => Value::Null,
        Bson::Boolean(b) => Value::Bool(*b),
        Bson::Int32(i) => Value::Int(*i as i64),
        Bson::Int64(i) => Value::Int(*i),
        Bson::Double(f) => Value::Float(*f),
        Bson::String(s) => Value::Text(s.clone()),
        Bson::ObjectId(oid) => Value::Text(oid.to_string()),
        Bson::DateTime(dt) => Value::Text(dt.to_string()),
        Bson::Timestamp(ts) => Value::Text(format!("Timestamp({})", ts.time)),
        Bson::Binary(bin) => Value::Text(format!("<binary:{}>", bin.bytes.len())),
        Bson::Decimal128(d) => Value::Text(d.to_string()),
        Bson::Symbol(s) => Value::Text(s.clone()),
        Bson::JavaScriptCode(s) => Value::Text(s.clone()),
        Bson::Document(doc) => {
            let json = serde_json::to_string(doc).unwrap_or_else(|_| "{}".to_string());
            Value::NestedDoc(json)
        }
        Bson::Array(arr) => {
            let json = serde_json::to_string(arr).unwrap_or_else(|_| "[]".to_string());
            Value::NestedArray(json)
        }
        other => Value::Text(other.to_string()),
    }
}
