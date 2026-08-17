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
    let text_columns = req.text_columns.clone().unwrap_or_else(|| columns.clone());
    for col in &text_columns {
        validate_identifier(col)?;
        if !columns.iter().any(|c| c == col) {
            bail!("text column '{}' is not present in '{}'", col, req.table);
        }
    }

    let select = columns
        .iter()
        .map(|c| quote(c))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {select} FROM {}", quote(&req.table));
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query([])?;
    let index = columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.as_str(), i))
        .collect::<HashMap<_, _>>();
    let mut current = Vec::new();
    while let Some(row) = rows.next()? {
        let values = (0..columns.len())
            .map(|i| value_string(row.get_ref(i).expect("column index")))
            .collect::<Vec<_>>();
        let key = req
            .key_columns
            .iter()
            .map(|c| values[index[c.as_str()]].clone())
            .collect::<Vec<_>>()
            .join("|");
        let text = text_columns
            .iter()
            .map(|c| format!("{c}={}", values[index[c.as_str()]]))
            .collect::<Vec<_>>()
            .join("; ");
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        let hash = format!("{:x}", hasher.finalize());
        let label = format!("sql://{}/{}#{}", req.database.display(), req.table, key);
        current.push(SqlRow {
            key,
            label,
            text,
            hash,
        });
    }
    drop(rows);
    drop(stmt);

    let previous: Manifest = std::fs::read(manifest_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut old_keys = previous.rows;
    let mut new_rows = Vec::new();
    let mut changed_rows = Vec::new();
    let mut unchanged = 0;
    let mut next = Manifest::default();
    for row in &current {
        next.rows.insert(
            row.key.clone(),
            ManifestRow {
                label: row.label.clone(),
                hash: row.hash.clone(),
            },
        );
        match old_keys.remove(&row.key) {
            None => new_rows.push(row.clone()),
            Some(old) if old.hash != row.hash => changed_rows.push(row.clone()),
            Some(_) => unchanged += 1,
        }
    }
    let deleted_rows = old_keys
        .into_iter()
        .map(|(key, old)| SqlRow {
            key,
            label: old.label,
            text: String::new(),
            hash: old.hash,
        })
        .collect::<Vec<_>>();

    if let Some(parent) = manifest_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = manifest_file.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&next)?)?;
    std::fs::rename(tmp, manifest_file)?;

    Ok(SqlDiff {
        new_rows,
        changed_rows,
        deleted_rows,
        unchanged,
        total: current.len(),
    })
}

#[cfg(not(feature = "sql"))]
pub fn scan(_req: &SqlRequest, _manifest_file: &Path) -> Result<SqlDiff> {
    bail!("SQL ingestion requires the 'sql' feature")
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
}
