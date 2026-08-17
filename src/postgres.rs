//! PostgreSQL row-delta ingestion.
//!
//! This connector deliberately shares the same `SqlDiff` contract as SQLite.
//! It reads through a read-only SQL query, converts rows to stable JSON-backed
//! text, and persists only hashes and labels locally. TLS should be supplied by
//! the deployment network or a local tunnel; the connector does not silently
//! downgrade a production connection to plaintext.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::sql::{SqlColumn, SqlDiff, SqlRow, SqlTable};

#[derive(Debug, Clone)]
pub struct Request {
    pub dsn: String,
    pub source: String,
    pub table: String,
    pub key_columns: Vec<String>,
    pub text_columns: Option<Vec<String>>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Default)]
struct Manifest {
    rows: HashMap<String, ManifestRow>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct ManifestRow {
    label: String,
    hash: String,
}

pub fn manifest_path(data_dir: &Path, user: &str, source: &str, table: &str) -> std::path::PathBuf {
    let mut h = Sha256::new();
    h.update(user.as_bytes());
    h.update([0]);
    h.update(source.as_bytes());
    h.update([0]);
    h.update(table.as_bytes());
    data_dir.join(format!("postgres_scan_{:x}.json", h.finalize()))
}

#[cfg(feature = "postgres")]
pub async fn schema(dsn: &str) -> Result<Vec<SqlTable>> {
    let (client, connection) = tokio_postgres::connect(dsn, tokio_postgres::NoTls)
        .await
        .context("connect to PostgreSQL")?;
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            log::error!("PostgreSQL connection failed: {error}");
        }
    });
    let rows = client
        .query(
            "SELECT table_name, column_name, data_type, is_nullable
             FROM information_schema.columns
             WHERE table_schema = 'public'
             ORDER BY table_name, ordinal_position",
            &[],
        )
        .await?;
    let mut tables: HashMap<String, Vec<SqlColumn>> = HashMap::new();
    for row in rows {
        let table: String = row.get(0);
        tables.entry(table).or_default().push(SqlColumn {
            name: row.get(1),
            declared_type: row.get(2),
            primary_key: false,
            not_null: row.get::<_, String>(3) == "NO",
        });
    }
    Ok(tables
        .into_iter()
        .map(|(name, columns)| SqlTable { name, columns })
        .collect())
}

#[cfg(not(feature = "postgres"))]
pub async fn schema(_dsn: &str) -> Result<Vec<SqlTable>> {
    bail!("PostgreSQL ingestion requires the 'postgres' feature")
}

#[cfg(feature = "postgres")]
pub async fn scan(req: &Request, manifest_file: &Path) -> Result<SqlDiff> {
    validate_identifier(&req.table)?;
    if req.key_columns.is_empty() {
        bail!("key_columns must contain at least one column");
    }
    for column in &req.key_columns {
        validate_identifier(column)?;
    }
    let (client, connection) = tokio_postgres::connect(&req.dsn, tokio_postgres::NoTls)
        .await
        .context("connect to PostgreSQL")?;
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            log::error!("PostgreSQL connection failed: {error}");
        }
    });
    let columns = client
        .query(
            "SELECT column_name FROM information_schema.columns
             WHERE table_schema = 'public' AND table_name = $1
             ORDER BY ordinal_position",
            &[&req.table],
        )
        .await?
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    if columns.is_empty() {
        bail!("table '{}' does not exist or has no columns", req.table);
    }
    for column in &req.key_columns {
        if !columns.iter().any(|item| item == column) {
            bail!("key column '{}' is not present in '{}'", column, req.table);
        }
    }
    let text_columns = req.text_columns.clone().unwrap_or_else(|| columns.clone());
    for column in &text_columns {
        validate_identifier(column)?;
        if !columns.iter().any(|item| item == column) {
            bail!("text column '{}' is not present in '{}'", column, req.table);
        }
    }
    let select = columns.iter().map(|column| quote(column)).collect::<Vec<_>>().join(", ");
    let query = format!("SELECT row_to_json(t)::text FROM (SELECT {select} FROM {}.{}) t", quote("public"), quote(&req.table));
    let rows = client.query(&query, &[]).await?;
    let indexes = columns.iter().enumerate().map(|(i, column)| (column.as_str(), i)).collect::<HashMap<_, _>>();
    let mut current = Vec::new();
    for row in rows {
        let json: Value = serde_json::from_str::<Value>(&row.get::<_, String>(0))?;
        let values = columns.iter().map(|column| json.get(column).cloned().unwrap_or(Value::Null)).collect::<Vec<_>>();
        let key = req.key_columns.iter().map(|column| canonical_value(&values[indexes[column.as_str()]])).collect::<Vec<_>>().join("|");
        let text = text_columns.iter().map(|column| format!("{column}={}", canonical_value(&values[indexes[column.as_str()]]))).collect::<Vec<_>>().join("; ");
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let label = format!("postgres://{}/{}#{}", req.source, req.table, key);
        current.push(SqlRow { key, label, text, hash });
    }
    let previous: Manifest = std::fs::read(manifest_file).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let mut old = previous.rows;
    let mut next = HashMap::new();
    let mut new_rows = Vec::new();
    let mut changed_rows = Vec::new();
    for row in &current {
        next.insert(row.key.clone(), ManifestRow { label: row.label.clone(), hash: row.hash.clone() });
        match old.remove(&row.key) {
            None => new_rows.push(row.clone()),
            Some(previous) if previous.hash != row.hash => changed_rows.push(row.clone()),
            Some(_) => {}
        }
    }
    let deleted_rows = old.into_iter().map(|(key, previous)| SqlRow { key, label: previous.label, text: String::new(), hash: previous.hash }).collect::<Vec<_>>();
    if let Some(parent) = manifest_file.parent() { std::fs::create_dir_all(parent)?; }
    let tmp = manifest_file.with_extension(format!("tmp{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&Manifest { rows: next })?)?;
    std::fs::rename(tmp, manifest_file)?;
    let unchanged = current.len().saturating_sub(new_rows.len() + changed_rows.len());
    Ok(SqlDiff { new_rows, changed_rows, deleted_rows, unchanged, total: current.len() })
}

#[cfg(not(feature = "postgres"))]
pub async fn scan(_req: &Request, _manifest_file: &Path) -> Result<SqlDiff> {
    bail!("PostgreSQL ingestion requires the 'postgres' feature")
}

fn quote(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn validate_identifier(identifier: &str) -> Result<()> {
    let mut chars = identifier.chars();
    match chars.next() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => bail!("invalid SQL identifier: {identifier}"),
    }
    if chars.any(|c| !(c == '_' || c.is_ascii_alphanumeric())) {
        bail!("invalid SQL identifier: {identifier}");
    }
    Ok(())
}

fn canonical_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => "NULL".into(),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}
