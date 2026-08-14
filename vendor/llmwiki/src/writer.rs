use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::time::SystemTime;

use anyhow::{anyhow, Result};

use crate::indexer;
use crate::parser;

#[derive(Debug)]
pub struct WriteResult {
    pub path: String,
    pub created: bool,
    pub reindexed: bool,
}

pub fn write_page(wiki_root: &Path, rel_path: &str, content: &str) -> Result<WriteResult> {
    if !rel_path.ends_with(".md") {
        return Err(anyhow!("Only .md files are allowed"));
    }

    if !is_safe_path(wiki_root, rel_path) {
        return Err(anyhow!("Path traversal denied"));
    }

    let meta = parser::parse_frontmatter(content).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("description") {
            anyhow!("Missing required frontmatter field: description")
        } else {
            anyhow!("Missing required frontmatter field: title")
        }
    })?;

    if meta.description.len() > 200 {
        eprintln!("Warning: description exceeds 200 characters for {}", rel_path);
    }

    let abs_path = wiki_root.join(rel_path);
    let created = !abs_path.exists();

    if let Some(parent) = abs_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp_path = PathBuf::from(format!("{}.tmp", abs_path.display()));
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(content.as_bytes())?;
        f.flush()?;
    }
    fs::rename(&tmp_path, &abs_path)?;

    append_write_log(wiki_root, rel_path, created)?;
    indexer::index_root(wiki_root, true)?;

    Ok(WriteResult {
        path: rel_path.to_string(),
        created,
        reindexed: true,
    })
}

pub fn is_safe_path(wiki_root: &Path, rel_path: &str) -> bool {
    let canonical_root = match wiki_root.canonicalize() {
        Ok(p) => p,
        Err(_) => return false,
    };

    let mut target = canonical_root.clone();
    for component in Path::new(rel_path).components() {
        match component {
            Component::Normal(c) => target.push(c),
            Component::ParentDir => {
                if !target.pop() || !target.starts_with(&canonical_root) {
                    return false;
                }
            }
            Component::CurDir => {}
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }

    target.starts_with(&canonical_root)
}

fn append_write_log(wiki_root: &Path, path: &str, created: bool) -> Result<()> {
    let log_path = wiki_root.join(".index").join("write_log.jsonl");
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let timestamp = format_timestamp(SystemTime::now());
    let action = if created { "create" } else { "update" };
    let entry = serde_json::json!({
        "timestamp": timestamp,
        "path": path,
        "action": action,
    });
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    writeln!(file, "{}", entry)?;
    Ok(())
}

// Civil date algorithm from Howard Hinnant (https://howardhinnant.github.io/date_algorithms.html)
fn format_timestamp(t: SystemTime) -> String {
    let secs = t
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let sec = secs % 60;
    let min = (secs / 60) % 60;
    let hour = (secs / 3600) % 24;
    let days = (secs / 86400) as i64;
    let z = days + 719468;
    let era = if z >= 0 { z / 146097 } else { (z - 146096) / 146097 };
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, min, sec
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    const VALID_PAGE: &str = "---\ntitle: Test Page\ndescription: A test description.\ntags: [test]\nconfidence: 0.8\n---\n\n# Test Page\n\nSome content here.\n";

    fn init(dir: &Path) {
        crate::indexer::ensure_index_dirs(dir).unwrap();
    }

    #[test]
    fn test_is_safe_path_valid() {
        let tmp = TempDir::new().unwrap();
        assert!(is_safe_path(tmp.path(), "bio/page.md"));
        assert!(is_safe_path(tmp.path(), "page.md"));
    }

    #[test]
    fn test_is_safe_path_traversal() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_safe_path(tmp.path(), "../outside.md"));
        assert!(!is_safe_path(tmp.path(), "bio/../../outside.md"));
    }

    #[test]
    fn test_is_safe_path_absolute() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_safe_path(tmp.path(), "/etc/passwd.md"));
    }

    #[test]
    fn test_write_page_invalid_extension() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path());
        let err = write_page(tmp.path(), "test.txt", "content").unwrap_err();
        assert!(err.to_string().contains("Only .md files are allowed"));
    }

    #[test]
    fn test_write_page_path_traversal() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path());
        let err = write_page(tmp.path(), "../outside.md", VALID_PAGE).unwrap_err();
        assert!(err.to_string().contains("Path traversal denied"));
    }

    #[test]
    fn test_write_page_missing_frontmatter() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path());
        let err = write_page(tmp.path(), "test.md", "# No Frontmatter\n\nContent.").unwrap_err();
        assert!(
            err.to_string().contains("frontmatter") || err.to_string().contains("Frontmatter")
        );
    }

    #[test]
    fn test_write_page_creates_file_and_log() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path());
        let result = write_page(tmp.path(), "bio/test.md", VALID_PAGE).unwrap();
        assert!(result.created);
        assert!(result.reindexed);
        assert!(tmp.path().join("bio/test.md").exists());
        let log_path = tmp.path().join(".index").join("write_log.jsonl");
        assert!(log_path.exists());
        let log = fs::read_to_string(&log_path).unwrap();
        assert!(log.contains("\"action\":\"create\""));
        assert!(log.contains("bio/test.md"));
    }

    #[test]
    fn test_write_page_update() {
        let tmp = TempDir::new().unwrap();
        init(tmp.path());
        write_page(tmp.path(), "bio/test.md", VALID_PAGE).unwrap();
        let result = write_page(tmp.path(), "bio/test.md", VALID_PAGE).unwrap();
        assert!(!result.created);
        let log = fs::read_to_string(tmp.path().join(".index").join("write_log.jsonl")).unwrap();
        assert!(log.contains("\"action\":\"update\""));
    }

    #[test]
    fn test_format_timestamp_epoch() {
        let epoch = SystemTime::UNIX_EPOCH;
        let ts = format_timestamp(epoch);
        assert_eq!(ts, "1970-01-01T00:00:00Z");
    }
}
