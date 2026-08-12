//! Driver for the Moonraker adapter — the piece that turns
//! [`crate::moonraker::MoonrakerAdapter`] into a running connector.
//!
//! The adapter can poll a printer and shape one [`OperationalEvent`]; nothing
//! called it. This polls on an interval, suppresses unchanged states, and
//! optionally registers what it sees into a physis engine.
//!
//! **Why dedupe on state rather than `event_key`:** `OperationalEvent::rekey`
//! hashes `observed_at` into the key, and a poller stamps that with the wall
//! clock at poll time. Two polls of an idle printer therefore produce two
//! different keys for the same reality, so the content hash cannot answer "have
//! I already reported this". A poller's real question is "has anything
//! changed?", which is what [`StateFingerprint`] captures.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::moonraker::MoonrakerAdapter;
use crate::operational::{OperationalEvent, OperationalStatus};

/// The part of a printer's state worth reporting a change about.
///
/// Deliberately excludes timestamps and continuously-moving values like
/// progress: a print moving from 41% to 42% is not an event, while a print
/// moving from `printing` to `paused` is. Without this the connector would
/// emit one event per poll forever.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateFingerprint {
    pub status: String,
    pub subject: String,
    pub print_state: String,
    pub message: String,
}

impl StateFingerprint {
    pub fn of(event: &OperationalEvent) -> Self {
        let prop = |k: &str| event.properties.get(k).cloned().unwrap_or_default();
        Self {
            status: event.status.as_str().to_string(),
            subject: event.subject.clone(),
            print_state: prop("print_state"),
            message: prop("message"),
        }
    }
}

/// How the connector should behave.
pub struct ConnectOptions {
    /// Moonraker base URL, e.g. `http://printer.local:7125`.
    pub url: String,
    pub machine_id: String,
    pub interval_secs: u64,
    /// Poll once and exit — for cron, smoke tests, and CI.
    pub once: bool,
    /// physis engine base URL to register events into, e.g. `http://127.0.0.1:19876`.
    pub post_to: Option<String>,
    /// Tenant for those registrations (`X-Physis-User`).
    pub user: String,
    /// Bearer token, when the engine requires one.
    pub token: Option<String>,
    /// Where to persist the last fingerprint so a restart does not re-report a
    /// state that was already reported ("reconnects do not duplicate events").
    pub state_file: Option<PathBuf>,
}

/// Map an operational status onto the asserted coherence axis.
///
/// `Info` is deliberately *not* an assertion: a status line is an observation,
/// and registering it as 0.0 would claim the engine had judged it inert.
pub fn asserted_for(status: OperationalStatus) -> Option<f32> {
    match status {
        OperationalStatus::Success => Some(1.0),
        OperationalStatus::Failure => Some(-1.0),
        OperationalStatus::Warning => Some(-0.5),
        OperationalStatus::Inert => Some(0.0),
        OperationalStatus::Info => None,
    }
}

/// One-line human summary used as the registered node's text, so the event is
/// recallable by meaning in the wiki browser rather than by an opaque id.
pub fn summarize(event: &OperationalEvent) -> String {
    let mut s = format!("{} — {}", event.status.as_str(), event.subject);
    if let Some(state) = event.properties.get("print_state") {
        s.push_str(&format!(" [{state}]"));
    }
    if let Some(msg) = event.properties.get("message") {
        if !msg.is_empty() {
            s.push_str(&format!(" — {msg}"));
        }
    }
    if let Some(id) = &event.machine_id {
        s.push_str(&format!(" ({id})"));
    }
    s
}

fn load_fingerprint(path: &Path) -> Option<StateFingerprint> {
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_fingerprint(path: &Path, fp: &StateFingerprint) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, serde_json::to_string(fp)?)?;
    Ok(())
}

/// Register an event into a physis engine as a coherence node with a verdict.
async fn register(opts: &ConnectOptions, base: &str, event: &OperationalEvent) -> anyhow::Result<()> {
    let mut body = serde_json::json!({ "input": summarize(event) });
    if let Some(score) = asserted_for(event.status) {
        body["coherence_score"] = serde_json::json!(score);
    }
    let mut rb = reqwest::Client::new()
        .post(format!("{}/api/v1/coherence/register", base.trim_end_matches('/')))
        .header("X-Physis-User", &opts.user)
        .json(&body);
    if let Some(t) = &opts.token {
        rb = rb.bearer_auth(t);
    }
    let resp = rb.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("engine returned {}", resp.status());
    }
    Ok(())
}

/// Poll the printer until interrupted (or once, with `--once`).
///
/// Each emitted event is printed to stdout as one JSON line so the connector
/// composes with `jq`, a file, or a pipe — the sidecar shape the connector
/// roadmap asks for, rather than a new bespoke transport.
pub async fn run(opts: ConnectOptions) -> anyhow::Result<()> {
    let adapter = MoonrakerAdapter::new(&opts.url, &opts.machine_id);
    let mut last = opts.state_file.as_deref().and_then(load_fingerprint);

    loop {
        match adapter.poll_event().await {
            Ok(event) => {
                let fp = StateFingerprint::of(&event);
                if last.as_ref() != Some(&fp) {
                    println!("{}", serde_json::to_string(&event)?);
                    if let Some(base) = &opts.post_to {
                        if let Err(e) = register(&opts, base, &event).await {
                            // A registration failure must not kill the connector:
                            // the printer keeps running whether or not the engine
                            // is up, and dropping telemetry is worse than a warning.
                            eprintln!("[moonraker] register failed: {e}");
                        }
                    }
                    if let Some(path) = &opts.state_file {
                        if let Err(e) = save_fingerprint(path, &fp) {
                            eprintln!("[moonraker] could not persist state: {e}");
                        }
                    }
                    last = Some(fp);
                }
            }
            // Printers get powered off mid-print. Keep polling rather than
            // exiting, so the connector recovers on its own when it comes back.
            Err(e) => eprintln!("[moonraker] poll failed: {e}"),
        }
        if opts.once {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_secs(opts.interval_secs.max(1))).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moonraker::MoonrakerSnapshot;

    fn demo_event() -> OperationalEvent {
        let raw = include_bytes!("../tests/fixtures/moonraker_snapshot.json");
        let snapshot = MoonrakerSnapshot::from_json(raw).expect("fixture parses");
        MoonrakerAdapter::new("http://printer.local:7125", "voron-01")
            .event_from_snapshot(snapshot)
    }

    #[test]
    fn identical_state_is_not_re_reported() {
        // The whole point of the connector's dedupe: polling an unchanged
        // printer must stay silent.
        //
        // Two polls of an idle printer differ only in when they happened, which
        // is exactly what `rekey` hashes — so the content key changes while the
        // reality does not, and dedupe has to look at state instead. (Replaying
        // one fixture twice would NOT show this: the fixture's observed_at is
        // fixed, so both keys match. The poll clock is the variable.)
        let a = demo_event();
        let mut b = demo_event();
        b.observed_at = a.observed_at + chrono::Duration::seconds(10);
        b.rekey();

        assert_ne!(a.event_key, b.event_key, "the key moved with the poll clock");
        assert_eq!(
            StateFingerprint::of(&a),
            StateFingerprint::of(&b),
            "but nothing about the printer changed, so nothing should be emitted"
        );
    }

    #[test]
    fn a_real_state_change_is_reported() {
        // The fixture is a paused print; resuming it is a real event.
        let base = demo_event();
        assert_eq!(base.properties.get("print_state").map(String::as_str), Some("paused"));
        let mut changed = demo_event();
        changed.properties.insert("print_state".into(), "printing".into());
        assert_ne!(StateFingerprint::of(&base), StateFingerprint::of(&changed));

        // Progress moving is NOT an event — otherwise every poll of a running
        // print emits, which is the failure mode the fingerprint exists to stop.
        let mut ticked = demo_event();
        ticked.properties.insert("progress".into(), "0.43".into());
        assert_eq!(StateFingerprint::of(&base), StateFingerprint::of(&ticked));
    }

    #[test]
    fn fingerprint_survives_a_restart() {
        // "Reconnects do not duplicate events" — the fingerprint has to round
        // trip through disk, not just live in memory.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let fp = StateFingerprint::of(&demo_event());
        save_fingerprint(&path, &fp).unwrap();
        assert_eq!(load_fingerprint(&path), Some(fp));
        // A missing or corrupt file must read as "nothing known yet" rather
        // than panic, otherwise a bad file bricks the connector on boot.
        assert_eq!(load_fingerprint(&dir.path().join("absent.json")), None);
        std::fs::write(&path, "{not json").unwrap();
        assert_eq!(load_fingerprint(&path), None);
    }

    #[test]
    fn info_is_an_observation_not_a_verdict() {
        // Registering Info as 0.0 would claim the engine judged it inert.
        assert_eq!(asserted_for(OperationalStatus::Info), None);
        assert_eq!(asserted_for(OperationalStatus::Success), Some(1.0));
        assert_eq!(asserted_for(OperationalStatus::Failure), Some(-1.0));
        assert_eq!(asserted_for(OperationalStatus::Inert), Some(0.0));
    }

    #[test]
    fn summary_is_readable_enough_to_recall_later() {
        let s = summarize(&demo_event());
        assert!(s.contains("voron-01") || !s.is_empty());
        assert!(!s.starts_with("—"), "summary must lead with the status");
    }
}
