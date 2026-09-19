//! Populate the operational corpus from REAL sources on this machine.
//!
//! `operational.rs` has defined the normalized event shape for a long time but
//! nothing ever filled it — `~/.physis-core/nodes.json` reports `nodes: 0`,
//! `edges: 0`. That emptiness is what blocks the episodic half of the depth-1
//! nesting proposal (Iteration 35): testing whether shared second-grade
//! structure corroborates a link needs entities observed in SEPARATE
//! encounters, and an ontology's own hints are not separate encounters.
//!
//! Everything here is real and already on disk. Nothing is synthesized, because
//! a synthetic corpus would answer the co-occurrence question with whatever
//! structure the generator happened to impose — which is the failure mode this
//! whole research track exists to avoid.
//!
//! Three connectors, one per `OperationalKind` that has a genuine source here:
//!
//!   * **Document** — markdown across physis-pro and physis-core, plus the
//!     `research_new` paper. One event per PARAGRAPH, not per file: an
//!     encounter is a context in which terms co-occur, and a whole file is too
//!     coarse to be one. Timestamps are the file's real mtime.
//!
//!   * **Machine** — randomesh's `auto_update.log`, genuine timer telemetry
//!     with ISO-8601 timestamps. Status is read from the line's own wording
//!     (`WARN`/`ERROR`/`failed` vs the rest), not assigned.
//!
//!   * **Workflow** — git history of both repos. Real author, real commit time,
//!     real subject, and the changed-file count as evidence.
//!
//! Output is a JSON array of `OperationalEvent`, which is exactly what
//! `physis timeline <file>` consumes, so the result is verifiable through the
//! existing CLI rather than only by this program's own say-so.
//!
//! Run:
//!   cargo run -p physis_pro --release --example populate_operational -- <out.json>

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, TimeZone, Utc};
use physis_pro::operational::{
    OperationalEvent, OperationalKind, OperationalProvenance, OperationalStatus,
};

fn mtime(path: &Path) -> DateTime<Utc> {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .map(DateTime::<Utc>::from)
        .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap())
}

/// Markdown paragraphs, with the nearest preceding heading as subject.
/// Code fences are skipped: a block of Rust is not an encounter between terms,
/// and letting it through would fill the corpus with syntax co-occurrence.
fn paragraphs(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut heading = String::from("(untitled)");
    let mut buf: Vec<&str> = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(h) = t.strip_prefix('#') {
            if !buf.is_empty() {
                out.push((heading.clone(), buf.join(" ")));
                buf.clear();
            }
            heading = h.trim_start_matches('#').trim().to_string();
            continue;
        }
        if t.is_empty() {
            if !buf.is_empty() {
                out.push((heading.clone(), buf.join(" ")));
                buf.clear();
            }
        } else {
            buf.push(t);
        }
    }
    if !buf.is_empty() {
        out.push((heading, buf.join(" ")));
    }
    // A one-line fragment is not a context worth calling an encounter.
    out.into_iter()
        .filter(|(_, b)| b.split_whitespace().count() >= 12)
        .collect()
}

fn collect_docs(roots: &[&str], out: &mut Vec<OperationalEvent>) {
    let mut files: Vec<PathBuf> = Vec::new();
    for r in roots {
        let p = Path::new(r);
        if p.is_file() {
            files.push(p.to_path_buf());
        } else if p.is_dir() {
            walk(p, &mut files, 0);
        }
    }
    files.sort();
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else {
            continue;
        };
        let when = mtime(&f);
        for (i, (heading, body)) in paragraphs(&text).into_iter().enumerate() {
            let mut prov = OperationalProvenance::new("ingest", "document");
            prov.path = Some(f.clone());
            prov.source_id = Some(format!("{}#{i}", f.display()));
            let mut ev = OperationalEvent::new(
                OperationalKind::Document,
                OperationalStatus::Info,
                when,
                heading,
                prov,
            );
            ev.target = Some(f.display().to_string());
            ev.project = Some("physis".into());
            ev.evidence = vec![body];
            out.push(ev);
        }
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    if depth > 3 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if name.starts_with('.') || name == "target" || name == "node_modules" || name == "vendor" {
            continue;
        }
        if p.is_dir() {
            walk(&p, out, depth + 1);
        } else if p.extension().and_then(|s| s.to_str()) == Some("md") {
            out.push(p);
        }
    }
}

/// randomesh timer telemetry: `[2026-09-07T09:26:37Z] physis: up to date`.
fn collect_machine(log: &Path, out: &mut Vec<OperationalEvent>) {
    let Ok(text) = std::fs::read_to_string(log) else {
        return;
    };
    for line in text.lines() {
        let Some(rest) = line.strip_prefix('[') else {
            continue;
        };
        let Some((ts, msg)) = rest.split_once("] ") else {
            continue;
        };
        let Ok(when) = DateTime::parse_from_rfc3339(ts) else {
            continue;
        };
        let msg = msg.trim();
        if msg.is_empty() {
            continue;
        }
        // Status comes from the line's own wording rather than being assigned.
        let status = if msg.contains("ERROR") || msg.contains("failed") || msg.contains("FAILED") {
            OperationalStatus::Failure
        } else if msg.contains("WARN") || msg.contains("skip") || msg.contains("refusing") {
            OperationalStatus::Warning
        } else {
            OperationalStatus::Success
        };
        let mut prov = OperationalProvenance::new("randomesh", "machine");
        prov.path = Some(log.to_path_buf());
        let mut ev = OperationalEvent::new(
            OperationalKind::Machine,
            status,
            when.with_timezone(&Utc),
            msg.chars().take(120).collect::<String>(),
            prov,
        );
        ev.machine_id = Some("feather".into());
        ev.project = Some("randomesh".into());
        ev.evidence = vec![msg.to_string()];
        out.push(ev);
    }
}

fn collect_commits(repo: &str, label: &str, out: &mut Vec<OperationalEvent>) {
    let Ok(o) = Command::new("git")
        .args([
            "-C",
            repo,
            "log",
            "--no-merges",
            "--date=unix",
            "--pretty=format:%H%x1f%ad%x1f%an%x1f%s%x1f%b%x1e",
        ])
        .output()
    else {
        return;
    };
    let text = String::from_utf8_lossy(&o.stdout);
    for rec in text.split('\u{1e}') {
        let f: Vec<&str> = rec.trim_start_matches('\n').split('\u{1f}').collect();
        if f.len() < 4 || f[0].is_empty() {
            continue;
        }
        let Ok(secs) = f[1].parse::<i64>() else {
            continue;
        };
        let when = Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now);
        let mut prov = OperationalProvenance::new("git", "workflow");
        prov.source_id = Some(f[0].to_string());
        prov.uri = Some(format!("{label}@{}", &f[0][..f[0].len().min(12)]));
        let mut ev = OperationalEvent::new(
            OperationalKind::Workflow,
            OperationalStatus::Success,
            when,
            f[3],
            prov,
        );
        ev.actor = Some(f[2].to_string());
        ev.target = Some(label.to_string());
        ev.project = Some(label.to_string());
        // The body carries the reasoning; that is the part with co-occurrence
        // structure worth having.
        if f.len() > 4 && !f[4].trim().is_empty() {
            ev.evidence = vec![f[4].trim().to_string()];
        }
        out.push(ev);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    // `--verify <file>` reads a corpus back through the real OperationalEvent
    // type. Serializing from the type only proves the writer agrees with
    // itself; a consumer has to deserialize, so that is what gets checked.
    if args.len() > 2 && args[1] == "--verify" {
        let text = std::fs::read_to_string(&args[2]).expect("read corpus");
        let events: Vec<OperationalEvent> =
            serde_json::from_str(&text).expect("deserialize as Vec<OperationalEvent>");
        let mut keys = std::collections::HashSet::new();
        let mut rekeyed_ok = 0usize;
        for e in &events {
            keys.insert(e.event_key.clone());
            let mut c = e.clone();
            c.rekey();
            if c.event_key == e.event_key {
                rekeyed_ok += 1;
            }
        }
        println!("round-trip OK: {} events deserialized", events.len());
        println!("unique event_key: {}", keys.len());
        println!(
            "content hash reproduces on rekey: {rekeyed_ok}/{}",
            events.len()
        );
        let ledger: std::collections::HashMap<_, usize> =
            events
                .iter()
                .fold(std::collections::HashMap::new(), |mut m, e| {
                    *m.entry(format!("{:?}", e.ledger_mechanism())).or_default() += 1;
                    m
                });
        let mut l: Vec<_> = ledger.into_iter().collect();
        l.sort();
        println!("causal-ledger mechanism split: {l:?}");
        return;
    }

    let out_path = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "operational_corpus.json".to_string());

    let mut events: Vec<OperationalEvent> = Vec::new();

    collect_docs(
        &[
            "research_new",
            "docs",
            "packets",
            "research",
            "physis-core/README.md",
            "physis-core/CHANGELOG.md",
            "physis-core/docs",
            "README.md",
            "PLAN.md",
            "PHYSIS_PRO_USECASES.md",
        ],
        &mut events,
    );
    let docs = events.len();

    collect_machine(
        Path::new("/home/gio/.local/state/randomesh/auto_update.log"),
        &mut events,
    );
    let machine = events.len() - docs;

    collect_commits(".", "physis-pro", &mut events);
    collect_commits("physis-core", "physis-core", &mut events);
    let workflow = events.len() - docs - machine;

    // Deterministic order: by time, then by the content hash the type computes
    // for itself. Never by collection order, which depends on directory reads.
    events.sort_by(|a, b| {
        a.observed_at
            .cmp(&b.observed_at)
            .then_with(|| a.event_key.cmp(&b.event_key))
    });
    // event_key is a content hash, so duplicates collapse honestly.
    let before = events.len();
    events.dedup_by(|a, b| a.event_key == b.event_key);

    let mut by_kind: HashMap<&str, usize> = HashMap::new();
    for e in &events {
        *by_kind.entry(e.kind.as_str()).or_default() += 1;
    }
    let words: usize = events
        .iter()
        .map(|e| {
            e.evidence
                .iter()
                .map(|s| s.split_whitespace().count())
                .sum::<usize>()
        })
        .sum();

    println!("collected  document={docs}  machine={machine}  workflow={workflow}");
    println!("deduped    {before} -> {} events", events.len());
    let mut kinds: Vec<_> = by_kind.into_iter().collect();
    kinds.sort();
    println!("by kind    {kinds:?}");
    println!("evidence   {words} words of real context");
    if let (Some(f), Some(l)) = (events.first(), events.last()) {
        println!("span       {} .. {}", f.observed_at, l.observed_at);
    }

    match serde_json::to_string_pretty(&events) {
        Ok(j) => {
            if let Err(e) = std::fs::write(&out_path, j) {
                eprintln!("write failed: {e}");
                std::process::exit(1);
            }
            println!("\nwrote {out_path} — verify with: physis timeline {out_path}");
        }
        Err(e) => {
            eprintln!("serialize failed: {e}");
            std::process::exit(1);
        }
    }
}
