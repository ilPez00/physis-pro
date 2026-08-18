//! Read-only SQL ingestion with row-level incremental diffs.
//!
//! The first connector is SQLite because it is zero-infrastructure and makes a
//! reliable local/demo fixture. The public contract is deliberately database-
//! shaped: stable row identity, content hash, changed/deleted rows, and a
//! manifest that can later be backed by a CDC cursor for Postgres/MySQL.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "sql")]
use rusqlite::{types::ValueRef, Connection, OpenFlags};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlRow {
    pub key: String,
    pub label: String,
    pub text: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Manifest {
    rows: HashMap<String, ManifestRow>,
    #[serde(default)]
    watermark: Option<Watermark>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Watermark {
    value: String,
    numeric: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ManifestRow {
    label: String,
    hash: String,
}

#[derive(Debug, Clone)]
pub struct SqlDiff {
    pub new_rows: Vec<SqlRow>,
    pub changed_rows: Vec<SqlRow>,
    pub deleted_rows: Vec<SqlRow>,
    pub unchanged: usize,
    pub total: usize,
}

#[derive(Debug, Clone)]
pub struct SqlRequest {
    pub database: PathBuf,
    pub table: String,
    pub key_columns: Vec<String>,
    pub text_columns: Option<Vec<String>>,
    pub updated_column: Option<String>,
    pub reconcile: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlColumn {
    pub name: String,
    pub declared_type: String,
    pub primary_key: bool,
    pub not_null: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlTable {
    pub name: String,
    pub columns: Vec<SqlColumn>,
}

pub fn manifest_path(data_dir: &Path, user: &str, database: &Path, table: &str) -> PathBuf {
    let mut h = Sha256::new();
    h.update(user.as_bytes());
    h.update([0]);
    h.update(database.to_string_lossy().as_bytes());
    h.update([0]);
    h.update(table.as_bytes());
    data_dir.join(format!("sql_scan_{:x}.json", h.finalize()))
}

#[cfg(feature = "sql")]
pub fn schema(database: &Path) -> Result<Vec<SqlTable>> {
    let conn = Connection::open_with_flags(
        database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open SQLite database read-only: {}", database.display()))?;
    let mut tables_stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = tables_stmt
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    tables
        .into_iter()
        .map(|name| {
            let columns = table_info(&conn, &name)?;
            Ok(SqlTable { name, columns })
        })
        .collect()
}

#[cfg(not(feature = "sql"))]
pub fn schema(_database: &Path) -> Result<Vec<SqlTable>> {
    bail!("SQL ingestion requires the 'sql' feature")
}

#[cfg(feature = "sql")]
pub fn scan(req: &SqlRequest, manifest_file: &Path) -> Result<SqlDiff> {
    validate_identifier(&req.table)?;
    if req.key_columns.is_empty() {
        bail!("key_columns must contain at least one column");
    }
    for col in &req.key_columns {
        validate_identifier(col)?;
    }

    let conn = Connection::open_with_flags(
        &req.database,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| format!("open SQLite database read-only: {}", req.database.display()))?;
    let columns = table_columns(&conn, &req.table)?;
    if columns.is_empty() {
        bail!("table '{}' does not exist or has no columns", req.table);
    }
    for col in &req.key_columns {
        if !columns.iter().any(|c| c == col) {
            bail!("key column '{}' is not present in '{}'", col, req.table);
        }
    }
    if let Some(col) = &req.updated_column {
        validate_identifier(col)?;
        if !columns.iter().any(|c| c == col) {
            bail!("updated column '{}' is not present in '{}'", col, req.table);
        }
    }
    let text_columns = req.text_columns.clone().unwrap_or_else(|| columns.clone());
    for col in &text_columns {
        validate_identifier(col)?;
        if !columns.iter().any(|c| c == col) {
            bail!("text column '{}' is not present in '{}'", col, req.table);
        }
    }

    let previous: Manifest = std::fs::read(manifest_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let incremental = !req.reconcile
        && req.updated_column.is_some()
        && previous.watermark.is_some();
    let current_keys = if incremental {
        read_keys(&conn, req)?
    } else {
        Vec::new()
    };
    let current = read_rows(
        &conn,
        req,
        &columns,
        &text_columns,
        if incremental { previous.watermark.as_ref() } else { None },
    )?;
    let source_watermark = req
        .updated_column
        .as_ref()
        .map(|column| read_watermark(&conn, &req.table, column))
        .transpose()?
        .flatten();
    let active_keys = if incremental {
        current_keys
    } else {
        current.iter().map(|row| row.key.clone()).collect()
    };
    let mut old_keys = previous.rows.clone();
    let mut new_rows = Vec::new();
    let mut changed_rows = Vec::new();
    let mut next = if incremental {
        previous.rows.clone()
    } else {
        HashMap::new()
    };
    for key in &active_keys {
        if !next.contains_key(key) {
            continue;
        }
        old_keys.remove(key);
    }
    for row in &current {
        let prior = old_keys.remove(&row.key).or_else(|| previous.rows.get(&row.key).cloned());
        next.insert(
            row.key.clone(),
            ManifestRow {
                label: row.label.clone(),
                hash: row.hash.clone(),
            },
        );
        match prior {
            None => new_rows.push(row.clone()),
            Some(old) if old.hash != row.hash => changed_rows.push(row.clone()),
            Some(_) => {}
        }
    }
    let deleted_rows = previous.rows
        .into_iter()
        .filter(|(key, _)| !active_keys.contains(key))
        .map(|(key, old)| SqlRow {
            key,
            label: old.label,
            text: String::new(),
            hash: old.hash,
        })
        .collect::<Vec<_>>();

    let next = Manifest { rows: next, watermark: source_watermark };
    crate::atomic::write_json_durable(manifest_file, &next)
        .with_context(|| format!("write scan manifest {}", manifest_file.display()))?;
    let unchanged = active_keys.len().saturating_sub(new_rows.len() + changed_rows.len());

    Ok(SqlDiff {
        new_rows,
        changed_rows,
        deleted_rows,
        unchanged,
        total: active_keys.len(),
    })
}

#[cfg(not(feature = "sql"))]
pub fn scan(_req: &SqlRequest, _manifest_file: &Path) -> Result<SqlDiff> {
    bail!("SQL ingestion requires the 'sql' feature")
}

#[cfg(feature = "sql")]
fn read_keys(conn: &Connection, req: &SqlRequest) -> Result<Vec<String>> {
    let select = req
        .key_columns
        .iter()
        .map(|column| quote(column))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {select} FROM {}", quote(&req.table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let mut keys = Vec::new();
    while let Some(row) = rows.next()? {
        let values = (0..req.key_columns.len())
            .map(|index| value_string(row.get_ref(index).expect("key index")))
            .collect::<Vec<_>>();
        keys.push(values.join("|"));
    }
    Ok(keys)
}

#[cfg(feature = "sql")]
fn read_rows(
    conn: &Connection,
    req: &SqlRequest,
    columns: &[String],
    text_columns: &[String],
    watermark: Option<&Watermark>,
) -> Result<Vec<SqlRow>> {
    let select = columns
        .iter()
        .map(|column| quote(column))
        .collect::<Vec<_>>()
        .join(", ");
    let filter = match (&req.updated_column, watermark) {
        (Some(column), Some(watermark)) if watermark.numeric => {
            format!(" WHERE (CAST({} AS REAL) >= CAST(?1 AS REAL) OR {} IS NULL)", quote(column), quote(column))
        }
        (Some(column), Some(_)) => {
            format!(" WHERE (CAST({} AS TEXT) >= CAST(?1 AS TEXT) OR {} IS NULL)", quote(column), quote(column))
        }
        _ => String::new(),
    };
    let sql = format!("SELECT {select} FROM {}{filter}", quote(&req.table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = match watermark {
        Some(watermark) if !filter.is_empty() => stmt.query([watermark.value.as_str()])?,
        _ => stmt.query([])?,
    };
    let index = columns
        .iter()
        .enumerate()
        .map(|(i, column)| (column.as_str(), i))
        .collect::<HashMap<_, _>>();
    let mut current = Vec::new();
    while let Some(row) = rows.next()? {
        let values = (0..columns.len())
            .map(|i| value_string(row.get_ref(i).expect("column index")))
            .collect::<Vec<_>>();
        let key = req
            .key_columns
            .iter()
            .map(|column| values[index[column.as_str()]].clone())
            .collect::<Vec<_>>()
            .join("|");
        let text = text_columns
            .iter()
            .map(|column| format!("{column}={}", values[index[column.as_str()]]))
            .collect::<Vec<_>>()
            .join("; ");
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let label = format!("sql://{}/{}#{}", req.database.display(), req.table, key);
        current.push(SqlRow { key, label, text, hash });
    }
    Ok(current)
}

#[cfg(feature = "sql")]
fn read_watermark(conn: &Connection, table: &str, column: &str) -> Result<Option<Watermark>> {
    let sql = format!("SELECT MAX({0}), typeof(MAX({0})) FROM {1}", quote(column), quote(table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let Some(row) = rows.next()? else { return Ok(None) };
    let value = row.get_ref(0)?;
    if matches!(value, ValueRef::Null) {
        return Ok(None);
    }
    let numeric = matches!(row.get::<_, String>(1)?.as_str(), "integer" | "real");
    Ok(Some(Watermark { value: value_string(value), numeric }))
}

#[cfg(feature = "sql")]
fn table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    Ok(table_info(conn, table)?
        .into_iter()
        .map(|column| column.name)
        .collect())
}

#[cfg(feature = "sql")]
fn table_info(conn: &Connection, table: &str) -> Result<Vec<SqlColumn>> {
    validate_identifier(table)?;
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", quote(table)))?;
    let mut rows = stmt.query([])?;
    let mut columns = Vec::new();
    while let Some(row) = rows.next()? {
        columns.push(SqlColumn {
            name: row.get(1)?,
            declared_type: row.get(2)?,
            not_null: row.get::<_, i64>(3)? != 0,
            primary_key: row.get::<_, i64>(5)? != 0,
        });
    }
    Ok(columns)
}

#[cfg(feature = "sql")]
fn value_string(value: ValueRef<'_>) -> String {
    match value {
        ValueRef::Null => "NULL".into(),
        ValueRef::Integer(v) => v.to_string(),
        ValueRef::Real(v) => v.to_string(),
        ValueRef::Text(v) => String::from_utf8_lossy(v).into_owned(),
        ValueRef::Blob(v) => format!(
            "blob:{}",
            Sha256::digest(v)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        ),
    }

}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return Err(anyhow!("invalid SQL identifier: {identifier}")),
    }
    if chars.any(|c| !(c == '_' || c.is_ascii_alphanumeric())) {
        return Err(anyhow!("invalid SQL identifier: {identifier}"));
    }
    Ok(())
}

#[cfg(all(test, feature = "sql"))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn detects_row_changes_and_deletes_without_reembedding_unchanged_rows() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("demo.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE orders (id INTEGER PRIMARY KEY, status TEXT, amount INTEGER); INSERT INTO orders VALUES (1, 'open', 10), (2, 'paid', 20);").unwrap();
        let req = SqlRequest {
            database: db.clone(),
            table: "orders".into(),
            key_columns: vec!["id".into()],
            text_columns: None,
            updated_column: None,
            reconcile: false,
        };
        let manifest = dir.path().join("manifest.json");
        let first = scan(&req, &manifest).unwrap();
        assert_eq!(
            (
                first.new_rows.len(),
                first.changed_rows.len(),
                first.unchanged
            ),
            (2, 0, 0)
        );
        conn.execute("UPDATE orders SET status='closed' WHERE id=1", [])
            .unwrap();
        conn.execute("DELETE FROM orders WHERE id=2", []).unwrap();
        let second = scan(&req, &manifest).unwrap();
        assert_eq!(
            (
                second.new_rows.len(),
                second.changed_rows.len(),
                second.deleted_rows.len(),
                second.unchanged
            ),
            (0, 1, 1, 0)
        );
        let third = scan(&req, &manifest).unwrap();
        assert_eq!(
            (
                third.changed_rows.len(),
                third.deleted_rows.len(),
                third.unchanged
            ),
            (0, 0, 1)
        );
    }

    #[test]
    fn watermark_reads_changed_payloads_but_reconciles_keys() {
        let dir = tempdir().unwrap();
        let db = dir.path().join("watermark.sqlite");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch("CREATE TABLE events (id INTEGER PRIMARY KEY, updated_at INTEGER, body TEXT); INSERT INTO events VALUES (1, 1, 'a'), (2, 2, 'b');").unwrap();
        let req = SqlRequest {
            database: db.clone(),
            table: "events".into(),
            key_columns: vec!["id".into()],
            text_columns: None,
            updated_column: Some("updated_at".into()),
            reconcile: false,
        };
        let manifest = dir.path().join("watermark.json");
        let first = scan(&req, &manifest).unwrap();
        assert_eq!((first.new_rows.len(), first.unchanged), (2, 0));
        conn.execute("UPDATE events SET updated_at=3, body='changed' WHERE id=1", []).unwrap();
        conn.execute("INSERT INTO events VALUES (3, 4, 'new')", []).unwrap();
        conn.execute("DELETE FROM events WHERE id=2", []).unwrap();
        let second = scan(&req, &manifest).unwrap();
        assert_eq!((second.new_rows.len(), second.changed_rows.len(), second.deleted_rows.len(), second.unchanged), (1, 1, 1, 0));
    }
}
