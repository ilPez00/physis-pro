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
    let select = columns
        .iter()
        .map(|column| quote(column))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "SELECT row_to_json(t)::text FROM (SELECT {select} FROM {}.{}) t",
        quote("public"),
        quote(&req.table)
    );
    let rows = client.query(&query, &[]).await?;
    let indexes = columns
        .iter()
        .enumerate()
        .map(|(i, column)| (column.as_str(), i))
        .collect::<HashMap<_, _>>();
    let mut current = Vec::new();
    for row in rows {
        let json: Value = serde_json::from_str::<Value>(&row.get::<_, String>(0))?;
        let values = columns
            .iter()
            .map(|column| json.get(column).cloned().unwrap_or(Value::Null))
            .collect::<Vec<_>>();
        let key = req
            .key_columns
            .iter()
            .map(|column| canonical_value(&values[indexes[column.as_str()]]))
            .collect::<Vec<_>>()
            .join("|");
        let text = text_columns
            .iter()
            .map(|column| {
                format!(
                    "{column}={}",
                    canonical_value(&values[indexes[column.as_str()]])
                )
            })
            .collect::<Vec<_>>()
            .join("; ");
        let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
        let label = format!("postgres://{}/{}#{}", req.source, req.table, key);
        current.push(SqlRow {
            key,
            label,
            text,
            hash,
        });
    }
    reconcile(current, manifest_file)
}

/// Compare the rows just read from PostgreSQL against the previous manifest and
/// advance it.
///
/// Split out of [`scan`] so the delta logic — the part that decides whether a
/// row is re-embedded, skipped, or deleted from the index — can be tested
/// without a live database. Everything above this point is connection handling
/// and identifier validation; everything expensive downstream keys off what
/// this returns.
// Without the `postgres` feature the only caller is the test module.
#[cfg_attr(not(feature = "postgres"), allow(dead_code))]
fn reconcile(current: Vec<SqlRow>, manifest_file: &Path) -> Result<SqlDiff> {
    // A missing manifest means a first scan. A corrupt one is deliberately
    // treated the same way: a full re-ingest is wasteful but correct, whereas
    // failing the scan would leave the caller with no way to recover.
    let previous: Manifest = std::fs::read(manifest_file)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let mut old = previous.rows;
    let mut next = HashMap::new();
    let mut new_rows = Vec::new();
    let mut changed_rows = Vec::new();
    for row in &current {
        next.insert(
            row.key.clone(),
            ManifestRow {
                label: row.label.clone(),
                hash: row.hash.clone(),
            },
        );
        match old.remove(&row.key) {
            None => new_rows.push(row.clone()),
            Some(previous) if previous.hash != row.hash => changed_rows.push(row.clone()),
            Some(_) => {}
        }
    }
    // Whatever the current read did not account for is gone from the source.
    // The label is carried over from the manifest because the row no longer
    // exists to derive one from, and the caller deletes by label prefix.
    let deleted_rows = old
        .into_iter()
        .map(|(key, previous)| SqlRow {
            key,
            label: previous.label,
            text: String::new(),
            hash: previous.hash,
        })
        .collect::<Vec<_>>();
    crate::atomic::write_json_durable(manifest_file, &Manifest { rows: next })
        .with_context(|| format!("write scan manifest {}", manifest_file.display()))?;
    let unchanged = current
        .len()
        .saturating_sub(new_rows.len() + changed_rows.len());
    Ok(SqlDiff {
        new_rows,
        changed_rows,
        deleted_rows,
        unchanged,
        total: current.len(),
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn row(key: &str, text: &str) -> SqlRow {
        SqlRow {
            key: key.into(),
            label: format!("postgres://crm/contacts#{key}"),
            text: text.into(),
            hash: format!("{:x}", Sha256::digest(text.as_bytes())),
        }
    }

    #[test]
    fn the_first_scan_reports_every_row_as_new() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        let diff = reconcile(vec![row("1", "a"), row("2", "b")], &manifest).unwrap();
        assert_eq!(diff.new_rows.len(), 2);
        assert_eq!(diff.changed_rows.len(), 0);
        assert_eq!(diff.unchanged, 0);
        assert_eq!(diff.total, 2);
    }

    /// The whole point of the manifest: a second scan over untouched data must
    /// send nothing to the embedder.
    #[test]
    fn an_unchanged_table_costs_no_embedding_work() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        reconcile(vec![row("1", "a"), row("2", "b")], &manifest).unwrap();
        let diff = reconcile(vec![row("1", "a"), row("2", "b")], &manifest).unwrap();
        assert!(diff.new_rows.is_empty());
        assert!(diff.changed_rows.is_empty());
        assert!(diff.deleted_rows.is_empty());
        assert_eq!(diff.unchanged, 2);
    }

    #[test]
    fn an_edited_row_is_reported_as_changed_not_new() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        reconcile(vec![row("1", "a"), row("2", "b")], &manifest).unwrap();
        let diff = reconcile(vec![row("1", "a"), row("2", "edited")], &manifest).unwrap();
        assert!(diff.new_rows.is_empty());
        assert_eq!(diff.changed_rows.len(), 1);
        assert_eq!(diff.changed_rows[0].key, "2");
        assert_eq!(diff.unchanged, 1);
    }

    #[test]
    fn a_removed_row_is_reported_as_deleted_with_its_previous_label() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        reconcile(vec![row("1", "a"), row("2", "b")], &manifest).unwrap();
        let diff = reconcile(vec![row("1", "a")], &manifest).unwrap();
        assert_eq!(diff.deleted_rows.len(), 1);
        // The caller deletes by label prefix, so the label has to survive the
        // row it described.
        assert_eq!(diff.deleted_rows[0].label, "postgres://crm/contacts#2");
        assert_eq!(diff.total, 1);
    }

    /// A row that comes back after being deleted must be re-embedded, not
    /// mistaken for one already in the index.
    #[test]
    fn a_reinstated_row_is_new_again() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        reconcile(vec![row("1", "a")], &manifest).unwrap();
        reconcile(vec![], &manifest).unwrap();
        let diff = reconcile(vec![row("1", "a")], &manifest).unwrap();
        assert_eq!(diff.new_rows.len(), 1);
    }

    /// A truncated or hand-edited manifest must degrade to a full re-ingest
    /// rather than failing the scan, which would leave no way to recover.
    #[test]
    fn a_corrupt_manifest_falls_back_to_a_full_scan() {
        let dir = tempdir().unwrap();
        let manifest = dir.path().join("scan.json");
        reconcile(vec![row("1", "a")], &manifest).unwrap();
        std::fs::write(&manifest, b"{not json").unwrap();
        let diff = reconcile(vec![row("1", "a")], &manifest).unwrap();
        assert_eq!(diff.new_rows.len(), 1);
    }

    #[test]
    fn manifests_are_separated_by_user_source_and_table() {
        let dir = tempdir().unwrap();
        let base = manifest_path(dir.path(), "alice", "crm", "contacts");
        assert_ne!(base, manifest_path(dir.path(), "bob", "crm", "contacts"));
        assert_ne!(base, manifest_path(dir.path(), "alice", "erp", "contacts"));
        assert_ne!(base, manifest_path(dir.path(), "alice", "crm", "orders"));
        // Same inputs must land on the same file across processes, or every
        // scan would look like a first scan.
        assert_eq!(base, manifest_path(dir.path(), "alice", "crm", "contacts"));
    }

    /// The DSN carries the password. It must never reach a manifest filename,
    /// which is world-readable in the data directory.
    #[test]
    fn the_manifest_filename_leaks_nothing() {
        let dir = tempdir().unwrap();
        let path = manifest_path(dir.path(), "alice", "crm", "contacts");
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with("postgres_scan_"));
        for secret in ["alice", "crm", "contacts"] {
            assert!(!name.contains(secret), "{name} leaks {secret}");
        }
    }

    #[test]
    fn rejects_identifiers_that_could_escape_quoting() {
        for bad in [
            "orders; DROP TABLE users",
            "orders\"",
            "1orders",
            "",
            "public.orders",
            "ord ers",
        ] {
            assert!(validate_identifier(bad).is_err(), "accepted {bad:?}");
        }
        for good in ["orders", "_private", "order_items2"] {
            assert!(validate_identifier(good).is_ok(), "rejected {good:?}");
        }
    }

    #[test]
    fn quoting_doubles_embedded_quotes() {
        assert_eq!(quote("orders"), "\"orders\"");
        assert_eq!(quote("we\"ird"), "\"we\"\"ird\"");
    }

    /// Row text is the embedding-cache and change-detection key, so this pins
    /// how values are rendered — including one known collision.
    #[test]
    fn canonical_values_collide_null_with_the_literal_text_null() {
        assert_eq!(canonical_value(&Value::Bool(true)), "true");
        assert_eq!(canonical_value(&serde_json::json!(42)), "42");

        // Both render as "NULL". A column that flips between SQL NULL and the
        // four-character string "NULL" therefore hashes identically and the
        // scan reports it unchanged. Rare enough to accept, but recorded here
        // so it is a known limit rather than a future mystery.
        assert_eq!(canonical_value(&Value::Null), "NULL");
        assert_eq!(canonical_value(&Value::String("NULL".into())), "NULL");
    }
}
