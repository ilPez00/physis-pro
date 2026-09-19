//! S0 exit gate: an `fs.read` through the shared action surface (the path the
//! HUD uses) returns the file and appends exactly one observation with
//! `ok=true`; an unknown verb is `unavailable` and appends exactly one
//! observation with `ok=false`. No silent success, no missing record.

use std::fs;

use physis_pro::action;
use physis_pro::capability::default_capabilities;
use physis_pro::nav::{NavIndex, ObjectId, ObjectKind, PhysisObject, SourceRef};
use physis_pro::registry::Registry;
use tempfile::TempDir;

fn file_object(path: &str, hash: &str) -> PhysisObject {
    PhysisObject {
        id: ObjectId::new("fs", hash),
        kind: ObjectKind::File,
        label: "note.md".to_string(),
        source: Some(SourceRef {
            path: path.to_string(),
            start_line: None,
            end_line: None,
        }),
        mtime: 0.0,
        hash: hash.to_string(),
        text: String::new(),
        embedding: Vec::new(),
    }
}

fn registry_with_file(content: &str) -> (TempDir, Registry, String) {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("note.md");
    fs::write(&path, content).unwrap();

    let obs = tmp.path().join("observations.jsonl");
    let mut reg = Registry::new(&obs);
    for cap in default_capabilities() {
        reg.add_capability(cap);
    }

    let obj = file_object(&path.to_string_lossy(), "abc123");
    let oid = obj.id.short();
    let mut index = NavIndex::new(tmp.path());
    index.objects.insert(oid.clone(), obj);
    reg.register_nav_index(&index);

    (tmp, reg, oid)
}

#[test]
fn read_through_the_action_surface_returns_content_and_one_ok_observation() {
    let (_tmp, mut reg, oid) = registry_with_file("hello action surface\n");

    let (result, rendered) =
        action::dispatch(&mut reg, &format!("read {oid}")).expect("well-formed line");

    assert!(result.ok, "read failed: {rendered}");
    assert_eq!(result.capability, "fs.read");
    assert!(
        rendered.contains("hello action surface"),
        "content not rendered: {rendered}"
    );

    let tail = reg.obs_tail(10);
    assert_eq!(tail.len(), 1, "expected exactly one observation");
    assert!(tail[0].ok);
    assert_eq!(tail[0].verb, "read");
    assert_eq!(tail[0].object, oid);
    assert_eq!(tail[0].capability.as_deref(), Some("fs.read"));
}

#[test]
fn unknown_verb_is_unavailable_and_records_ok_false() {
    let (_tmp, mut reg, oid) = registry_with_file("irrelevant\n");

    let (result, rendered) =
        action::dispatch(&mut reg, &format!("frobnicate {oid}")).expect("well-formed line");

    assert!(!result.ok, "unknown verb must not succeed: {rendered}");
    assert_eq!(result.error.as_deref(), Some("unavailable"));
    assert_eq!(result.capability, "none");

    let tail = reg.obs_tail(10);
    assert_eq!(tail.len(), 1, "expected exactly one observation");
    assert!(!tail[0].ok);
    assert!(tail[0].capability.is_none());
}

#[test]
fn malformed_line_is_rejected_before_any_call() {
    let (_tmp, mut reg, _oid) = registry_with_file("x\n");
    assert!(action::dispatch(&mut reg, "read").is_err());
    assert!(action::dispatch(&mut reg, "read obj limit").is_err());
    assert_eq!(reg.obs_tail(10).len(), 0, "no call means no observation");
}

#[test]
fn host_and_device_placeholders_exist_and_are_honestly_unavailable() {
    let (_tmp, mut reg, _oid) = registry_with_file("x\n");
    reg.register_placeholders();

    let kinds: Vec<String> = reg.list_objects().into_iter().map(|(_, k, _)| k).collect();
    assert!(kinds.contains(&"host".to_string()));
    assert!(kinds.contains(&"device".to_string()));

    let (result, _) = action::dispatch(&mut reg, "inspect host:local").expect("well-formed");
    assert!(!result.ok);
    assert_eq!(result.error.as_deref(), Some("unavailable"));
}

#[test]
fn irreversible_verb_without_confirmation_is_refused_and_recorded() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join("work");
    fs::create_dir_all(&dir).unwrap();
    let obs = tmp.path().join("observations.jsonl");
    let mut reg = Registry::new(&obs);
    for cap in default_capabilities() {
        reg.add_capability(cap);
    }

    let obj = PhysisObject {
        id: ObjectId::new("fs", "d1"),
        kind: ObjectKind::Directory,
        label: "work".to_string(),
        source: Some(SourceRef {
            path: dir.to_string_lossy().to_string(),
            start_line: None,
            end_line: None,
        }),
        mtime: 0.0,
        hash: "d1".to_string(),
        text: String::new(),
        embedding: Vec::new(),
    };
    let oid = obj.id.short();
    let mut index = NavIndex::new(tmp.path());
    index.objects.insert(oid.clone(), obj);
    reg.register_nav_index(&index);

    // No confirm: refused, recorded ok=false, nothing executed.
    let (result, rendered) =
        action::dispatch(&mut reg, &format!("execute {oid} cmd=echo")).expect("well-formed");
    assert!(!result.ok, "unconfirmed execute must not run: {rendered}");
    assert_eq!(result.error.as_deref(), Some("approval_required"));
    let tail = reg.obs_tail(10);
    assert_eq!(tail.len(), 1, "the refusal is recorded");
    assert!(!tail[0].ok);

    // With confirm: it runs.
    let (result, rendered) =
        action::dispatch(&mut reg, &format!("execute {oid} cmd=echo confirm=true"))
            .expect("well-formed");
    assert!(result.ok, "confirmed execute should run: {rendered}");
}
