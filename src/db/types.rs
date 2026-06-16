#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    #[allow(dead_code)]
    pub type_name: String,
}

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct Row {
    pub values: Vec<Value>,
}

#[derive(Debug, Clone)]
pub struct DbQueryResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Row>,
    #[allow(dead_code)]
    pub rows_affected: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableKind {
    Table,
    View,
}

#[derive(Debug, Clone)]
pub struct TableObject {
    pub name: String,
    pub kind: TableKind,
}

#[derive(Debug, Clone)]
pub struct ForeignKey {
    pub table: String,
    pub column: String,
}

#[derive(Debug, Clone)]
pub struct ColumnSchema {
    pub name: String,
    pub type_name: String,
    pub is_pk: bool,
    #[allow(dead_code)]
    pub is_nullable: bool,
    pub fk: Option<ForeignKey>,
}

/// Format a BigDecimal for display: strips trailing zeros but keeps at least 2 decimal places.
/// PostgreSQL/MySQL encode NUMERIC/DECIMAL in base-10000 groups internally, so `10.69`
/// often comes back as `10.6900`. This normalises it: `10.6900` → `10.69`, `12.9000` → `12.90`.
pub fn format_decimal(d: bigdecimal::BigDecimal) -> String {
    let s = d.to_string();
    if let Some(dot_pos) = s.find('.') {
        let frac = &s[dot_pos + 1..];
        let sig = frac.trim_end_matches('0').len().max(2);
        let display_frac: String = frac.chars()
            .chain(std::iter::repeat('0'))
            .take(sig)
            .collect();
        format!("{}.{}", &s[..dot_pos], display_frac)
    } else {
        s
    }
}

/// Typed value of a Redis key, fetched via TYPE + the appropriate read command.
#[derive(Debug, Clone)]
pub enum KvKeyDetail {
    String(String),
    Hash(Vec<(String, String)>),   // (field, value) sorted by field
    List(Vec<String>),              // ordered values
    Set(Vec<String>),               // unordered members
    ZSet(Vec<(String, f64)>),      // (member, score) ordered by score
}
