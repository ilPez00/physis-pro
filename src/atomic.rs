//! Durable file replacement.
//!
//! Several subsystems persist state that must survive an interrupted write:
//! the embedding cache, the salience store, and the SQL scan manifests. They
//! had each grown their own version of the same temp-file-then-rename dance,
//! and the two newest copies had dropped the `sync_all` along the way. This
//! module is the single implementation they now share.

use std::io;
use std::path::Path;

/// Replace `target` with `bytes`, atomically for readers and durably for
/// crashes.
///
/// A plain [`std::fs::write`] truncates the target before writing, so an
/// interrupted or failing write leaves a short or empty file where valid state
/// used to be. That is not hypothetical here: a full disk truncated a warmed
/// embedding cache to zero bytes, and because the error was discarded the only
/// symptom was the next run silently re-embedding everything.
///
/// The sequence is:
///
/// 1. Write a temp file in the same directory, so the rename cannot cross a
///    filesystem boundary and stays atomic.
/// 2. `sync_all` it, so a short write on a full disk surfaces as an error here
///    rather than as corrupt state on the next run.
/// 3. Rename over the target. A reader sees either the old contents or the new
///    ones, never a torn mixture.
///
/// The temp name carries the pid so two concurrent writers do not collide, and
/// is removed on every failure path so a failed write does not leave litter.
///
/// The rename itself is only guaranteed durable once the *directory* is synced;
/// see [`write_durable`] for that stronger, more expensive variant.
pub fn write_atomic(target: &Path, bytes: &[u8]) -> io::Result<()> {
    write_impl(target, bytes, false)
}

/// [`write_atomic`], plus an fsync of the containing directory.
///
/// Renaming a file makes the new contents visible immediately, but the
/// directory entry recording that rename can still be lost in a power failure —
/// leaving the *old* file in place even though the write returned success. For
/// a cache that is a non-event, since it is rebuilt. For a scan manifest it is
/// not: losing the entry rewinds the manifest and re-ingests rows that were
/// already processed.
///
/// The extra fsync costs a disk round trip, so it is opt-in rather than the
/// default.
pub fn write_durable(target: &Path, bytes: &[u8]) -> io::Result<()> {
    write_impl(target, bytes, true)
}

fn write_impl(target: &Path, bytes: &[u8], sync_dir: bool) -> io::Result<()> {
    use std::io::Write as _;

    let dir = target.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;

    let tmp = target.with_extension(format!("tmp{}", std::process::id()));

    // Scoped so the file is closed before the rename.
    {
        let mut f = std::fs::File::create(&tmp)?;
        if let Err(e) = f.write_all(bytes).and_then(|_| f.sync_all()) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
    }

    std::fs::rename(&tmp, target).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })?;

    if sync_dir {
        // A directory sync is not portable. Where it is unsupported the write
        // is still atomic, which is the property callers depend on, so a
        // failure here is not worth failing the write over.
        if let Ok(handle) = std::fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
    Ok(())
}

/// Serialise `value` as JSON and [`write_durable`] it.
///
/// Serialisation failures are surfaced as `InvalidData` rather than panicking,
/// so a caller that returns `io::Result` needs no extra error plumbing.
pub fn write_json_durable<T: serde::Serialize>(target: &Path, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    write_durable(target, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn replaces_existing_contents() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("state.json");
        write_atomic(&target, b"first").unwrap();
        write_atomic(&target, b"second").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"second");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("nested").join("deeper").join("state.json");
        write_durable(&target, b"payload").unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"payload");
    }

    /// The point of the temp file is that a failure never damages what is
    /// already on disk. A read-only directory blocks creating the temp file,
    /// which is the reachable failure in a test; a full disk fails later, at
    /// the `write_all`, but leaves the same guarantee because the rename is
    /// what publishes the new contents.
    #[test]
    #[cfg(unix)]
    fn a_failed_write_leaves_the_previous_contents_intact() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let sub = dir.path().join("locked");
        std::fs::create_dir(&sub).unwrap();
        let target = sub.join("state.json");
        write_atomic(&target, b"original").unwrap();

        // Read and execute, but not write: the temp file cannot be created.
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o500)).unwrap();
        let result = write_atomic(&target, b"replacement");
        std::fs::set_permissions(&sub, std::fs::Permissions::from_mode(0o700)).unwrap();

        assert!(
            result.is_err(),
            "write into a read-only directory must fail"
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn leaves_no_temp_files_behind() {
        let dir = tempdir().unwrap();
        write_durable(&dir.path().join("state.json"), b"payload").unwrap();
        let leftovers = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("tmp"))
            .collect::<Vec<_>>();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn round_trips_json() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("value.json");
        write_json_durable(&target, &vec!["a", "b"]).unwrap();
        let back: Vec<String> = serde_json::from_slice(&std::fs::read(&target).unwrap()).unwrap();
        assert_eq!(back, vec!["a".to_string(), "b".to_string()]);
    }
}
