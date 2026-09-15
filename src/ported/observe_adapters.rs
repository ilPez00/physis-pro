// provenance: moved from physis-core src/observe.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-15. One implementation; no fork.
//! Collection adapters for the observation log.
//!
//! `physis_core::observe` keeps the append-only [`Observation`] log, its log
//! format and the claim path (`promote`/`cited_by`); these watchers are the
//! product-side collectors that turn a host resource — a filesystem tree,
//! git history, a shell history file, an agent's own session store, `/proc`,
//! a browser's SQLite store — into `Observation`s for that log. Each watcher
//! holds no state beyond what it reads, so they are independent, cheap to
//! add, and impossible to get wrong in a way that damages the log. `recall`
//! (Go) reaches the same conclusion from the other direction: read-only
//! adapters into stores you do not own, writes only ever to your own.
//!
//! Signatures, doc comments and behaviour are unchanged from Core; only the
//! module boundary moved, per `physis-core/MOVED_TO_PRODUCT.md` ("Watchers").

use physis_core::observe::Observation;
use std::path::{Path, PathBuf};

/// Watch a directory tree: emit one observation per file whose content hash
/// differs from the last observation of that path.
///
/// Hash-gating before doing anything expensive is OpenRecall's instinct
/// (`is_similar` before embedding) applied at the right level: content, not
/// pixels. A file touched but unchanged produces nothing, so re-running this is
/// close to free and the log does not fill with noise.
pub fn watch_fs(root: &Path, known: &[Observation], max_files: usize) -> Vec<Observation> {
    use sha2::{Digest, Sha256};
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            if seen >= max_files {
                return out;
            }
            let p = e.path();
            let name = e.file_name().to_string_lossy().to_string();
            // Skip the noise every tree has. Not a security boundary — a
            // signal-to-noise one.
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            let Ok(meta) = p.metadata() else { continue };
            if meta.len() > 2_000_000 {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&p) else { continue };
            seen += 1;
            let hash = format!("{:x}", Sha256::digest(body.as_bytes()));
            let subject = p.to_string_lossy().to_string();
            let unchanged = known
                .iter()
                .rev()
                .find(|o| o.source == "fs" && o.subject == subject)
                .map(|o| o.body == hash)
                .unwrap_or(false);
            if unchanged {
                continue;
            }
            out.push(Observation::new("fs", subject).with_body(hash).by("watch_fs"));
        }
    }
    out
}

/// Watch git history: one observation per commit, with its duration spanning
/// back to the previous commit.
///
/// The duration is the point. A commit is an instant, but the *work* it records
/// is the interval since the last one — which makes [`concurrent`] able to
/// answer "what files were being edited while this commit was being written".
pub fn watch_git(repo: &Path, limit: usize) -> Vec<Observation> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["log", &format!("-{limit}"), "--format=%H%x1f%aI%x1f%s"])
        .output();
    let Ok(out) = out else { return Vec::new() };
    if !out.status.success() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let mut rows: Vec<(String, chrono::DateTime<chrono::Utc>, String)> = Vec::new();
    for line in text.lines() {
        let mut f = line.split('\u{1f}');
        let (Some(h), Some(t), Some(s)) = (f.next(), f.next(), f.next()) else { continue };
        let Ok(at) = chrono::DateTime::parse_from_rfc3339(t) else { continue };
        rows.push((h.to_string(), at.with_timezone(&chrono::Utc), s.to_string()));
    }
    // `git log` is newest-first; the gap to the NEXT row is the previous commit.
    let mut obs = Vec::new();
    for (i, (hash, at, subject)) in rows.iter().enumerate() {
        let dur = rows
            .get(i + 1)
            .map(|(_, prev, _)| (*at - *prev).num_milliseconds().max(0) as u64);
        let mut o = Observation::new("git", hash.clone())
            .with_body(subject.clone())
            .by("watch_git");
        o.at = *at;
        o.duration_ms = dur;
        obs.push(o);
    }
    obs.reverse(); // oldest first, so sequence order matches time order
    obs
}

/// Watch shell history: one observation per command, with its real runtime.
///
/// zsh's extended history is `: <epoch>:<elapsed>;<command>`, and `<elapsed>` is
/// the seconds the command actually ran. That is a genuine `duration_ms` rather
/// than an inferred one, which makes [`concurrent`] able to answer *"what was I
/// running while that file changed"* precisely instead of approximately.
///
/// bash history has no timestamps unless `HISTTIMEFORMAT` is set, so plain
/// lines are taken as instants rather than guessed at.
///
/// Read-only, like `recall`'s adapters into other tools' stores: this never
/// writes to the history file it reads.
pub fn watch_shell(hist: &Path, known: &[Observation], limit: usize) -> Vec<Observation> {
    // Lossy, not `read_to_string`. zsh writes metafied bytes for characters
    // outside ASCII, so a real history file is frequently NOT valid UTF-8 and
    // `read_to_string` returns Err — which this watcher would have reported as
    // "nothing changed". Silent emptiness from a source that plainly has
    // content is the worst failure shape available to a watcher, and it is what
    // shipped: the unit test used clean ASCII and never hit it.
    let Ok(bytes) = std::fs::read(hist) else { return Vec::new() };
    let text = String::from_utf8_lossy(&bytes);
    let seen: std::collections::HashSet<&str> = known
        .iter()
        .filter(|o| o.source == "terminal")
        .map(|o| o.subject.as_str())
        .collect();

    // zsh writes a multi-line command as physical lines joined by a trailing
    // backslash. Read line-by-line, each continuation becomes a separate
    // timestamp-less "command" that was never run. Join them first.
    let mut joined: Vec<String> = Vec::new();
    for line in text.lines() {
        match joined.last_mut() {
            Some(prev) if prev.ends_with('\\') => {
                prev.pop();
                prev.push(' ');
                prev.push_str(line);
            }
            _ => joined.push(line.to_string()),
        }
    }

    let mut out = Vec::new();
    for line in joined.iter().rev().take(limit * 4) {
        let line = line.as_str();
        let (at, elapsed, cmd) = match line.strip_prefix(": ") {
            Some(rest) => {
                let Some((meta, cmd)) = rest.split_once(';') else { continue };
                let Some((ts, el)) = meta.split_once(':') else { continue };
                let Ok(ts) = ts.trim().parse::<i64>() else { continue };
                (
                    chrono::DateTime::from_timestamp(ts, 0),
                    el.trim().parse::<u64>().ok(),
                    cmd,
                )
            }
            None if !line.trim().is_empty() => (None, None, line),
            _ => continue,
        };
        let cmd = cmd.trim();
        // A command line is its own identity here; re-running the watcher must
        // not re-append what is already recorded.
        if cmd.is_empty() || seen.contains(cmd) || out.iter().any(|o: &Observation| o.subject == cmd) {
            continue;
        }
        let mut o = Observation::new("terminal", cmd).by("watch_shell");
        if let Some(t) = at {
            o.at = t;
        }
        // Zero-second commands are instants, not zero-length intervals.
        o.duration_ms = elapsed.filter(|e| *e > 0).map(|e| e * 1000);
        out.push(o);
        if out.len() >= limit {
            break;
        }
    }
    out.reverse(); // oldest first, so sequence order matches time order
    out
}

/// Watch an agent's own session store: one observation per turn.
///
/// This is the source the brief calls *model outputs*, and it is the one that
/// makes the substrate reflexive — the machine observes what it was asked and
/// what it answered, on the same timeline as the files it changed and the
/// commands it ran.
///
/// Read-only over JSONL, `recall`'s adapter discipline: writes go only to our
/// own log. Unparseable lines are skipped rather than fatal, because a session
/// being written to concurrently will always have a partial final line.
pub fn watch_agent(dir: &Path, known: &[Observation], limit: usize) -> Vec<Observation> {
    let seen: std::collections::HashSet<&str> = known
        .iter()
        .filter(|o| o.source == "agent")
        .map(|o| o.subject.as_str())
        .collect();

    let mut files: Vec<PathBuf> = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|x| x.to_str()) == Some("jsonl") {
                files.push(p);
            }
        }
    }
    // Newest sessions first: the recent ones are the ones worth observing.
    files.sort_by_key(|p| {
        std::cmp::Reverse(
            p.metadata().and_then(|m| m.modified()).ok(),
        )
    });

    let mut out = Vec::new();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for line in text.lines() {
            if out.len() >= limit {
                out.reverse();
                return out;
            }
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            let (Some(ts), Some(sid)) = (
                v.get("timestamp").and_then(|x| x.as_str()),
                v.get("sessionId").and_then(|x| x.as_str()),
            ) else {
                continue;
            };
            let Ok(at) = chrono::DateTime::parse_from_rfc3339(ts) else { continue };
            // Subject is session+timestamp: stable, and unique per turn.
            let subject = format!("{}@{}", &sid[..8.min(sid.len())], ts);
            if seen.contains(subject.as_str()) {
                continue;
            }
            let kind = v.get("type").and_then(|x| x.as_str()).unwrap_or("turn");
            let body: String = v
                .get("content")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .chars()
                .take(400)
                .collect();
            let mut o = Observation::new("agent", subject)
                .with_body(format!("[{kind}] {body}"))
                .by("watch_agent");
            o.at = at.with_timezone(&chrono::Utc);
            out.push(o);
        }
    }
    out.reverse();
    out
}

/// Watch running processes: one observation per long-lived process, with its
/// real age as the duration.
///
/// Reads `/proc` directly — no dependency, no shelling out. `starttime` (field
/// 22 of `/proc/<pid>/stat`, in clock ticks since boot) plus the system boot
/// time gives when the process began, and the elapsed time to now is a genuine
/// interval.
///
/// `min_age_s` exists because a substrate that records every `ls` learns
/// nothing. Short-lived processes are noise; a build that has been running for
/// four minutes is a fact about what the machine is doing.
pub fn watch_proc(known: &[Observation], min_age_s: u64, limit: usize) -> Vec<Observation> {
    let Ok(uptime) = std::fs::read_to_string("/proc/uptime") else { return Vec::new() };
    let Some(up) = uptime.split_whitespace().next().and_then(|x| x.parse::<f64>().ok()) else {
        return Vec::new();
    };
    let now = chrono::Utc::now();
    let boot = now - chrono::Duration::milliseconds((up * 1000.0) as i64);
    // Linux reports `starttime` in clock ticks; USER_HZ is 100 on every
    // mainstream configuration, and getting it exactly is not worth a libc dep
    // for a field used only to order and age processes.
    const TICKS_PER_SEC: f64 = 100.0;

    let seen: std::collections::HashSet<&str> = known
        .iter()
        .filter(|o| o.source == "process")
        .map(|o| o.subject.as_str())
        .collect();

    let Ok(rd) = std::fs::read_dir("/proc") else { return Vec::new() };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        let Ok(pid) = name.parse::<u32>() else { continue };
        let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else { continue };
        // comm is parenthesised and may itself contain spaces, so split after
        // the closing paren rather than on whitespace from the start.
        let Some(close) = stat.rfind(')') else { continue };
        let comm = stat[stat.find('(').map(|i| i + 1).unwrap_or(0)..close].to_string();
        let fields: Vec<&str> = stat[close + 1..].split_whitespace().collect();
        let Some(start_ticks) = fields.get(19).and_then(|x| x.parse::<f64>().ok()) else {
            continue;
        };
        let started = boot + chrono::Duration::milliseconds(
            (start_ticks / TICKS_PER_SEC * 1000.0) as i64,
        );
        let age_ms = (now - started).num_milliseconds().max(0) as u64;
        if age_ms < min_age_s * 1000 {
            continue;
        }
        // pid+start identifies this run: a recycled pid is a different process.
        let subject = format!("{comm}#{pid}@{}", started.timestamp());
        if seen.contains(subject.as_str()) {
            continue;
        }
        let cmdline = std::fs::read_to_string(format!("/proc/{pid}/cmdline"))
            .map(|c| c.replace('\0', " ").trim().to_string())
            .unwrap_or_default();
        let mut o = Observation::new("process", subject)
            .with_body(cmdline.chars().take(200).collect::<String>())
            .by("watch_proc");
        o.at = started;
        o.duration_ms = Some(age_ms);
        out.push(o);
        if out.len() >= limit {
            break;
        }
    }
    out.sort_by_key(|o| o.at);
    out
}

/// Watch browser history: one observation per visited page.
///
/// Firefox `places.sqlite` and Chromium `History` are both SQLite, and this
/// crate has no SQLite dependency by design. Rather than take one for a
/// watcher, it shells out to the `sqlite3` CLI in **read-only immutable** mode,
/// which is also the only safe way to read a database the browser has open.
///
/// Returns nothing — and says nothing — when `sqlite3` is absent. That is a
/// missing capability, not an error: the rest of the substrate is unaffected.
pub fn watch_browser(db: &Path, known: &[Observation], limit: usize) -> Vec<Observation> {
    let firefox = db.file_name().and_then(|n| n.to_str()) == Some("places.sqlite");
    // Firefox stores microseconds since epoch; Chromium stores microseconds
    // since 1601-01-01, hence the 11644473600s offset.
    let sql = if firefox {
        format!(
            "SELECT url, COALESCE(title,''), last_visit_date/1000000 FROM moz_places \
             WHERE last_visit_date IS NOT NULL ORDER BY last_visit_date DESC LIMIT {limit};"
        )
    } else {
        format!(
            "SELECT url, COALESCE(title,''), last_visit_time/1000000-11644473600 FROM urls \
             WHERE last_visit_time > 0 ORDER BY last_visit_time DESC LIMIT {limit};"
        )
    };
    let uri = format!("file:{}?immutable=1", db.display());
    let Ok(out) = std::process::Command::new("sqlite3")
        .args(["-readonly", "-separator", "\u{1f}", &uri, &sql])
        .output()
    else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }

    let seen: std::collections::HashSet<&str> = known
        .iter()
        .filter(|o| o.source == "browser")
        .map(|o| o.subject.as_str())
        .collect();

    let text = String::from_utf8_lossy(&out.stdout);
    let mut obs = Vec::new();
    for line in text.lines() {
        let mut f = line.split('\u{1f}');
        let (Some(url), Some(title), Some(ts)) = (f.next(), f.next(), f.next()) else {
            continue;
        };
        if url.is_empty() || seen.contains(url) {
            continue;
        }
        let Ok(ts) = ts.trim().parse::<i64>() else { continue };
        let Some(at) = chrono::DateTime::from_timestamp(ts, 0) else { continue };
        let mut o = Observation::new("browser", url).with_body(title).by("watch_browser");
        o.at = at;
        obs.push(o);
    }
    obs.reverse(); // oldest first
    obs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("physis-pro-obs-adapters-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Re-running a watcher over an unchanged tree must produce nothing. If this
    /// regresses, continuous observation becomes unaffordable and the log fills
    /// with noise that means nothing.
    #[test]
    fn the_fs_watcher_is_silent_when_nothing_changed() {
        let d = tmp("fs");
        std::fs::write(d.join("a.txt"), "hello").unwrap();
        let first = watch_fs(&d, &[], 100);
        assert_eq!(first.len(), 1);
        let second = watch_fs(&d, &first, 100);
        assert!(second.is_empty(), "unchanged tree must produce no observations");

        std::fs::write(d.join("a.txt"), "changed").unwrap();
        let third = watch_fs(&d, &first, 100);
        assert_eq!(third.len(), 1, "a content change must be seen");
    }

    /// zsh extended history carries the REAL runtime of each command, which is
    /// what makes cross-source overlap precise rather than inferred.
    #[test]
    fn the_shell_watcher_reads_real_durations_and_dedupes() {
        let d = tmp("shell");
        let h = d.join("hist");
        std::fs::write(
            &h,
            ": 1700000000:12;cargo test\n: 1700000100:0;ls\n: 1700000200:5;git push\n",
        )
        .unwrap();
        let first = watch_shell(&h, &[], 10);
        assert_eq!(first.len(), 3);
        let ct = first.iter().find(|o| o.subject == "cargo test").unwrap();
        assert_eq!(ct.duration_ms, Some(12_000), "12 elapsed seconds");
        let ls = first.iter().find(|o| o.subject == "ls").unwrap();
        assert_eq!(ls.duration_ms, None, "a zero-second command is an instant");
        // Oldest first, so sequence order matches time order.
        assert_eq!(first[0].subject, "cargo test");
        // Re-running must not duplicate.
        assert!(watch_shell(&h, &first, 10).is_empty());
    }

    /// A session file being written to concurrently always has a partial final
    /// line. That must cost one turn, not the whole session.
    /// A multi-line command is one command. Read as physical lines, each
    /// continuation becomes a timestamp-less entry for something that was never
    /// run as written.
    #[test]
    fn the_shell_watcher_joins_continuation_lines() {
        let d = tmp("shell-cont");
        let h = d.join("hist");
        std::fs::write(
            &h,
            ": 1700000000:2;cargo build \\\n  --release \\\n  --features cli\n: 1700000100:0;ls\n",
        )
        .unwrap();
        let obs = watch_shell(&h, &[], 10);
        assert_eq!(obs.len(), 2, "one joined command plus ls, not four fragments");
        let built = obs.iter().find(|o| o.subject.starts_with("cargo build")).unwrap();
        assert!(built.subject.contains("--release"), "got {:?}", built.subject);
        assert!(built.subject.contains("--features cli"));
        assert_eq!(built.duration_ms, Some(2_000));
    }

    /// A real zsh history is frequently not valid UTF-8. The watcher must read
    /// it anyway rather than reporting "nothing changed", which is what it did
    /// until this test existed.
    #[test]
    fn the_shell_watcher_survives_invalid_utf8() {
        use std::io::Write;
        let d = tmp("shell-utf8");
        let h = d.join("hist");
        let mut f = std::fs::File::create(&h).unwrap();
        f.write_all(b": 1700000000:3;echo ").unwrap();
        f.write_all(&[0xE9, 0x83]).unwrap(); // metafied bytes, invalid UTF-8
        f.write_all(b"\n: 1700000100:0;ls\n").unwrap();
        drop(f);
        assert!(std::fs::read_to_string(&h).is_err(), "the fixture must be invalid UTF-8");

        let obs = watch_shell(&h, &[], 10);
        assert_eq!(obs.len(), 2, "both commands must be read despite the bad bytes");
        assert!(obs.iter().any(|o| o.subject == "ls"));
    }

    #[test]
    fn the_agent_watcher_skips_partial_lines() {
        let d = tmp("agent");
        std::fs::write(
            d.join("s.jsonl"),
            "{\"timestamp\":\"2026-09-12T10:00:00Z\",\"sessionId\":\"abcdef123\",\"type\":\"user\",\"content\":\"hello\"}\n             {\"timestamp\":\"2026-09-12T10:01:00Z\",\"sessionId\":\"abcdef123\",\"type\":\"assistant\",\"content\":\"hi\"}\n             {\"timestamp\":\"not-fin",
        )
        .unwrap();
        let obs = watch_agent(&d, &[], 10);
        assert_eq!(obs.len(), 2, "two good turns, one partial line skipped");
        assert!(obs.iter().all(|o| o.source == "agent"));
        assert!(obs[0].body.contains("[user]") || obs[1].body.contains("[user]"));
        assert!(watch_agent(&d, &obs, 10).is_empty(), "must dedupe on re-run");
    }

    /// Short-lived processes are noise; a build running for minutes is a fact.
    /// The age gate is what keeps continuous observation affordable.
    #[test]
    fn the_process_watcher_gates_on_age() {
        // This test's own process has been alive for well under an hour.
        let long = watch_proc(&[], 3600, 50);
        let short = watch_proc(&[], 0, 50);
        assert!(
            short.len() >= long.len(),
            "a lower age gate cannot return fewer processes"
        );
        for o in &short {
            assert_eq!(o.source, "process");
            assert!(o.duration_ms.is_some(), "a process age is a real interval");
            assert!(o.subject.contains('#'), "pid must be in the identity: {}", o.subject);
        }
    }

    /// A recycled pid is a different process, so identity must include when it
    /// started — otherwise re-running the watcher silently merges two runs.
    #[test]
    fn process_identity_includes_start_time() {
        let obs = watch_proc(&[], 0, 5);
        if let Some(o) = obs.first() {
            assert!(o.subject.contains('@'), "identity needs a start stamp: {}", o.subject);
            // Re-running with these known must produce nothing for them.
            let again = watch_proc(&obs, 0, 5);
            assert!(again.iter().all(|n| !obs.iter().any(|k| k.subject == n.subject)));
        }
    }

    /// A missing `sqlite3` is a missing capability, not a failure: the watcher
    /// returns nothing and the rest of the substrate is unaffected.
    #[test]
    fn the_browser_watcher_is_quiet_when_the_db_is_absent() {
        let d = tmp("browser");
        assert!(watch_browser(&d.join("places.sqlite"), &[], 10).is_empty());
    }
}
