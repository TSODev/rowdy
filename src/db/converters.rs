use crate::db::types::{Column, ColumnSchema, DbQueryResult, KvKeyDetail, Row, Value};

// ── KV → DbQueryResult ────────────────────────────────────────────────────────

pub fn kv_detail_to_result(detail: KvKeyDetail) -> DbQueryResult {
    let col = |n: &str| Column { name: n.to_string(), type_name: String::new() };
    let txt = |s: String| Value::Text(s);

    match detail {
        KvKeyDetail::String(v) => DbQueryResult {
            columns: vec![col("value")],
            rows: vec![Row { values: vec![txt(v)] }],
            rows_affected: 0,
        },
        KvKeyDetail::Hash(pairs) => DbQueryResult {
            columns: vec![col("field"), col("value")],
            rows: pairs.into_iter()
                .map(|(f, v)| Row { values: vec![txt(f), txt(v)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::List(items) => DbQueryResult {
            columns: vec![col("index"), col("value")],
            rows: items.into_iter().enumerate()
                .map(|(i, v)| Row { values: vec![Value::Int(i as i64), txt(v)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::Set(members) => DbQueryResult {
            columns: vec![col("member")],
            rows: members.into_iter()
                .map(|m| Row { values: vec![txt(m)] })
                .collect(),
            rows_affected: 0,
        },
        KvKeyDetail::ZSet(pairs) => DbQueryResult {
            columns: vec![col("member"), col("score")],
            rows: pairs.into_iter()
                .map(|(m, s)| Row { values: vec![txt(m), Value::Float(s)] })
                .collect(),
            rows_affected: 0,
        },
    }
}

// ── JSON ↔ Value ──────────────────────────────────────────────────────────────

pub fn json_val_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null       => Value::Null,
        serde_json::Value::Bool(b)    => Value::Bool(*b),
        serde_json::Value::Number(n)  => {
            if let Some(i) = n.as_i64() { Value::Int(i) }
            else { Value::Float(n.as_f64().unwrap_or(0.0)) }
        }
        serde_json::Value::String(s)  => Value::Text(s.clone()),
        serde_json::Value::Object(_)  => Value::NestedDoc(v.to_string()),
        serde_json::Value::Array(_)   => Value::NestedArray(v.to_string()),
    }
}

pub fn json_value_type_and_str(v: &serde_json::Value) -> (String, String) {
    match v {
        serde_json::Value::Bool(b)   => ("bool".into(), b.to_string()),
        serde_json::Value::Number(n) => {
            if n.is_f64() {
                ("float".into(), n.to_string())
            } else {
                ("int".into(), n.to_string())
            }
        }
        serde_json::Value::String(s) => ("string".into(), s.clone()),
        serde_json::Value::Null      => ("string".into(), "NULL".into()),
        serde_json::Value::Object(_) => ("object".into(), v.to_string()),
        serde_json::Value::Array(_)  => ("array".into(), v.to_string()),
    }
}

/// Convert a JSON string into a `DbQueryResult`, preserving nested objects/arrays
/// as `Value::NestedDoc` / `Value::NestedArray` for recursive navigation.
pub fn json_to_result(json: &str, is_array: bool) -> DbQueryResult {
    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v)  => v,
        Err(_) => return DbQueryResult {
            columns: vec![Column { name: "value".into(), type_name: "json".into() }],
            rows:    vec![Row { values: vec![Value::Text(json.into())] }],
            rows_affected: 0,
        },
    };

    if is_array {
        let items = match parsed.as_array() {
            Some(a) => a.clone(),
            None    => return DbQueryResult { columns: vec![], rows: vec![], rows_affected: 0 },
        };
        if items.is_empty() {
            return DbQueryResult { columns: vec![], rows: vec![], rows_affected: 0 };
        }
        if items[0].is_object() {
            // Array of objects → union of keys as columns, one row per item
            let mut col_names: Vec<String> = Vec::new();
            let mut seen = std::collections::HashSet::new();
            for item in &items {
                if let Some(obj) = item.as_object() {
                    for key in obj.keys() {
                        if seen.insert(key.clone()) {
                            col_names.push(key.clone());
                        }
                    }
                }
            }
            let columns: Vec<Column> = col_names.iter()
                .map(|n| Column { name: n.clone(), type_name: "json".into() })
                .collect();
            let rows: Vec<Row> = items.iter()
                .map(|item| Row {
                    values: col_names.iter()
                        .map(|cn| item.get(cn).map(json_val_to_value).unwrap_or(Value::Null))
                        .collect(),
                })
                .collect();
            DbQueryResult { columns, rows, rows_affected: 0 }
        } else {
            // Scalar array → index + value
            let columns = vec![
                Column { name: "index".into(), type_name: "int".into() },
                Column { name: "value".into(), type_name: "json".into() },
            ];
            let rows: Vec<Row> = items.iter().enumerate()
                .map(|(i, v)| Row { values: vec![Value::Int(i as i64), json_val_to_value(v)] })
                .collect();
            DbQueryResult { columns, rows, rows_affected: 0 }
        }
    } else {
        // Single object → one row, one column per key
        let obj = match parsed.as_object() {
            Some(o) => o,
            None => return DbQueryResult {
                columns: vec![Column { name: "value".into(), type_name: "json".into() }],
                rows:    vec![Row { values: vec![json_val_to_value(&parsed)] }],
                rows_affected: 0,
            },
        };
        let columns: Vec<Column> = obj.keys()
            .map(|k| Column { name: k.clone(), type_name: "json".into() })
            .collect();
        let values: Vec<Value> = obj.values().map(json_val_to_value).collect();
        DbQueryResult { columns, rows: vec![Row { values }], rows_affected: 0 }
    }
}

// ── MongoDB helpers ───────────────────────────────────────────────────────────

pub fn mongo_type_name(v: &Value) -> String {
    match v {
        Value::Bool(_)        => "bool",
        Value::Int(_)         => "int",
        Value::Float(_)       => "float",
        Value::NestedDoc(_)   => "object",
        Value::NestedArray(_) => "array",
        _                     => "string",
    }.to_string()
}

pub fn value_to_string(v: &Value) -> String {
    match v {
        Value::Null                              => "NULL".into(),
        Value::Bool(b)                           => b.to_string(),
        Value::Int(i)                            => i.to_string(),
        Value::Float(f)                          => f.to_string(),
        Value::Text(s)                           => s.clone(),
        Value::Bytes(b)                          => format!("<{} bytes>", b.len()),
        Value::NestedDoc(s) | Value::NestedArray(s) => s.clone(),
    }
}

// ── EditRecord schema inference ───────────────────────────────────────────────

pub fn json_object_to_schema_values(json: &str) -> (Vec<ColumnSchema>, Vec<String>) {
    let obj = match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(json) {
        Ok(o) => o,
        Err(_) => return (vec![], vec![]),
    };
    let mut schema = vec![];
    let mut values = vec![];
    for (key, val) in &obj {
        let (type_name, str_val) = json_value_type_and_str(val);
        schema.push(ColumnSchema { name: key.clone(), type_name, is_pk: false, is_nullable: true, fk: None });
        values.push(str_val);
    }
    (schema, values)
}

pub fn json_array_to_schema_values(json: &str) -> (Vec<ColumnSchema>, Vec<String>) {
    let arr = match serde_json::from_str::<Vec<serde_json::Value>>(json) {
        Ok(a) => a,
        Err(_) => return (vec![], vec![]),
    };
    let mut schema = vec![];
    let mut values = vec![];
    for (i, val) in arr.iter().enumerate() {
        let (type_name, str_val) = json_value_type_and_str(val);
        schema.push(ColumnSchema { name: format!("[{i}]"), type_name, is_pk: false, is_nullable: true, fk: None });
        values.push(str_val);
    }
    (schema, values)
}
