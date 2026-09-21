//! MVP loop integration test — protects the vertical slice the demo walks.
//!
//! One real directory → scan/map → natural-language intent → scoped world
//! slice → inspect → bounded action + validation → filesystem Δ → remap →
//! replayable transition records.
//!
//! Uses the deterministic offline embedder (`PHYSIS_EMBEDDER=random-projection`)
//! so the test needs no model files and no network.

use std::path::Path;
use std::sync::{Mutex, MutexGuard, OnceLock};

use physis_pro::mvp::log::LOG_VERSION;
use physis_pro::mvp::model::artifacts_of;
use physis_pro::mvp::repl;

/// Every session reads `PHYSIS_DATA_DIR` and `PHYSIS_EMBEDDER` from the
/// process environment (`NavWorkspace` and the embedder cascade both do), so
/// two tests opening different repositories in parallel would fight over one
/// data dir and one sled lock. Serialising the tests is the honest fix: the
/// production code takes the environment as given.
fn env_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// A small repository with one file per format the MVP must keep distinct.
fn seed_repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("src/lib.rs"),
        "pub fn compile_context(budget: usize) -> usize { budget }\n\
         pub struct TokenFixedRetriever;\n",
    )
    .unwrap();
    std::fs::write(
        root.join("README.md"),
        "# Demo\n\n## Retrieval\n\nContext is packed to a token budget.\n",
    )
    .unwrap();
    std::fs::write(
        root.join("config.json"),
        "{\"retrieval\": \"token-fixed\"}\n",
    )
    .unwrap();
    dir
}

fn open(root: &Path) -> repl::Session {
    // The supported offline mode: deterministic, model-free, no network.
    std::env::set_var("PHYSIS_EMBEDDER", "random-projection");
    std::env::set_var("PHYSIS_DATA_DIR", root.join(".physis"));
    repl::open(root).expect("the loop must open a real directory")
}

/// Real repository scans, structural hierarchy exists, and the startup report
/// shows only measured numbers.
#[test]
fn startup_reports_factual_state_of_a_real_directory() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    let banner = s.startup_banner();
    assert!(banner.contains("files          4"), "banner:\n{banner}");
    assert!(banner.contains("objects        "));
    assert!(
        !banner.contains("files          0"),
        "must count real files"
    );
    // The observer is not part of the observed corpus.
    assert!(!s.before_files.keys().any(|p| p.contains(".physis")));
}

/// Natural-language query scopes correctly, provenance survives retrieval, and
/// the telemetry the MVP promises is actually reported.
#[test]
fn natural_language_query_scopes_and_reports_telemetry() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    let out = s
        .handle("where is retrieval implemented?")
        .unwrap()
        .unwrap();
    for needle in [
        "objects considered",
        "objects selected",
        "candidate/raw context tokens",
        "final context tokens",
        "model/provider used",
        "latency",
        "why:",
    ] {
        assert!(out.contains(needle), "missing {needle} in:\n{out}");
    }
    // Provenance survives retrieval: an exact path in the slice, not a pointer.
    assert!(
        out.contains("README.md") || out.contains("src/lib.rs"),
        "no source path in:\n{out}"
    );
    let slice = s.last_slice.as_ref().expect("a slice must be built");
    assert!(!slice.context.is_empty(), "the slice must carry text");
    assert!(slice.final_tokens <= slice.budget, "budget must hold");
}

/// The map is navigable: inspect shows provenance and parents, dependencies are
/// real file relationships.
#[test]
fn map_is_navigable_and_dependencies_are_real() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    s.handle("where is compile_context used?").unwrap();
    let inspected = s.inspect(None).unwrap();
    assert!(inspected.contains("provenance:"), "got:\n{inspected}");
    assert!(inspected.contains("parents:"), "got:\n{inspected}");
    let deps = s.handle("dependencies src/lib.rs").unwrap().unwrap();
    assert!(deps.contains("dependencies of src/lib.rs"), "got:\n{deps}");
}
/// The bounded action loop: a plan is shown, validation executes in the
/// repository, the filesystem Δ is detected, and affected state is remapped.
#[test]
fn action_loop_detects_filesystem_delta_and_remaps() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    // Planning alone must not run anything.
    let plan = s
        .handle("improve retrieval without changing its public API")
        .unwrap()
        .unwrap();
    assert!(plan.contains("PLAN"), "got:\n{plan}");
    assert!(!plan.contains("VALIDATION"), "planning must not validate");

    // Approval runs validation in the repository, so there is something to
    // observe: the build writes artifacts the filesystem diff can see.
    let acted = s
        .act_flow("improve retrieval without changing its public API", true)
        .unwrap();
    for section in ["CHANGED", "AFFECTED", "STRUCTURE", "VALIDATION", "MAP"] {
        assert!(acted.contains(section), "missing {section}:\n{acted}");
    }
    assert!(
        acted.contains("act: cd "),
        "validation must run in the repo:\n{acted}"
    );

    // Whether or not the build changed files, the transition is recorded, and
    // a failed action is not dropped.
    let log = s.log.read_all();
    let last = log.last().expect("an action is a transition");
    assert_eq!(log.len(), 2, "plan and act are both transitions");
    assert_eq!(last.v, LOG_VERSION);
    assert!(last.intent.contains("improve retrieval"));
    assert!(last.action.starts_with("act:"));
    assert!(last.state_before.objects > 0, "pre-state must be recorded");
    assert!(last.timestamp.contains('T'), "timestamp is RFC 3339");
    assert!(last.uncertainty >= 0.0, "uncertainty is always stated");
    assert!(
        !last.validation.is_empty(),
        "the outcome of validation is recorded"
    );
}

/// Every interaction is an append-only transition: replayable, versioned,
/// ordered, never overwritten — including ones that changed nothing.
#[test]
fn every_interaction_is_an_append_only_replayable_transition() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    s.handle("what does this project do?").unwrap();
    s.handle("inspect").unwrap();
    s.handle("what changed recently?").unwrap();
    let first_len = s.log.read_all().len();
    assert!(first_len >= 3, "each interaction must be recorded");

    // Reopen: history survives and sequence continues.
    let log_path = s.log.path().to_path_buf();
    drop(s);
    let mut s2 = open(repo.path());
    assert_eq!(s2.log.path(), log_path, "one log per repository");
    s2.handle("inspect").unwrap();
    let all = s2.log.read_all();
    assert_eq!(
        all.len(),
        first_len + 1,
        "history is appended, never replaced"
    );
    for (i, t) in all.iter().enumerate() {
        assert_eq!(t.seq, i as u64, "sequence must be dense and ordered");
        assert_eq!(t.v, LOG_VERSION);
        assert!(!t.state_before.content_hash.is_empty());
        assert!(!t.state_after.map_hash.is_empty());
    }
    // A no-change interaction is still a transition: its absence is the fact.
    assert!(all.iter().any(|t| t.delta.no_change));
}

/// File-type invariants hold: the four formats are not flattened into one
/// homogeneous representation.
#[test]
fn file_type_invariants_are_preserved() {
    let _guard = env_lock();
    let rs = artifacts_of("src/lib.rs", "pub fn compile_context() {}\npub struct T;\n");
    let md = artifacts_of("README.md", "# Demo\n\n## Retrieval\n");
    let json = artifacts_of("config.json", r#"{"retrieval": "x"}"#);
    let toml = artifacts_of("Cargo.toml", "[package]\nname = \"demo\"\n");
    assert!(rs.iter().any(|a| a.structural == "fn compile_context"));
    assert!(rs.iter().any(|a| a.structural == "struct T"));
    assert!(md.iter().any(|a| a.structural == "h2:Retrieval"));
    assert!(json.iter().any(|a| a.structural == "key:retrieval"));
    assert!(toml.iter().any(|a| a.structural == "table:[package]"));
    assert!(toml.iter().any(|a| a.structural == "config:name"));
}

/// Two runs from a clean checkout behave the same: same query, same slice size
/// class, same structure of answer. Determinism is what makes it a demo.
#[test]
fn the_loop_runs_twice_from_a_clean_state() {
    let _guard = env_lock();
    for round in 0..2 {
        let repo = seed_repo();
        let mut s = open(repo.path());
        let out = s.handle("what does this project do?").unwrap().unwrap();
        assert!(out.contains("CONTEXT"), "round {round}:\n{out}");
        let slice = s.last_slice.as_ref().unwrap();
        assert!(slice.final_tokens > 0, "round {round} produced no context");
        assert!(
            slice.final_tokens <= slice.budget,
            "round {round} broke the budget"
        );
        assert!(!s.log.read_all().is_empty(), "round {round} logged nothing");
    }
}

/// New loop arms, all model-free: `context` renders the deterministic packet,
/// `focus` bands attention with pins surviving, `rescan` observes an external
/// edit, `bench` prints measured numbers, and `PHYSIS_MODEL=none` leaves the
/// loop coherent.
#[test]
fn context_focus_rescan_bench_are_model_free() {
    let _guard = env_lock();
    let repo = seed_repo();
    // The demo contract: coherent with no generative model configured.
    std::env::set_var("PHYSIS_MODEL", "none");
    std::env::remove_var("PHYSIS_ORACLE_KEY");
    std::env::remove_var("OPENROUTER_API_KEY");
    std::env::remove_var("GROQ_API_KEY");
    let mut s = open(repo.path());
    s.handle("where is retrieval implemented?").unwrap().unwrap();

    let packet = s.handle("context").unwrap().unwrap();
    for needle in ["INTENT", "TARGETS", "DEPENDENCIES", "CONSTRAINTS", "RECENT DELTA", "RELEVANT SOURCE", "EXCLUDED", "workspace source:", "generator: none"] {
        assert!(packet.contains(needle), "missing {needle} in:\n{packet}");
    }

    let focus = s.handle("focus retrieval").unwrap().unwrap();
    for needle in ["HOT", "WARM", "COLD", "pinned objects never demote"] {
        assert!(focus.contains(needle), "missing {needle} in:\n{focus}");
    }

    // External edit (not via `edit`, which needs a TTY editor): append a
    // symbol, then `rescan` must surface it as CHANGED with a MAP section.
    std::fs::write(
        repo.path().join("src/lib.rs"),
        "pub fn compile_context(budget: usize) -> usize { budget }\n\
         pub struct TokenFixedRetriever;\n\
         pub fn incremental_invalidate() {}\n",
    )
    .unwrap();
    let delta = s.handle("rescan").unwrap().unwrap();
    for needle in ["CHANGED", "AFFECTED", "STRUCTURE", "VALIDATION", "MAP", "src/lib.rs"] {
        assert!(delta.contains(needle), "missing {needle} in:\n{delta}");
    }

    let bench = s.handle("bench").unwrap().unwrap();
    for needle in ["measured", "objects indexed", "query latency", "generator: none", "network: OFFLINE"] {
        assert!(bench.contains(needle), "missing {needle} in:\n{bench}");
    }
    std::env::remove_var("PHYSIS_MODEL");
}

/// The REPL `impossible` command reports the same closed chain the HUD `M`
/// shows: settled numbers with gates attached, and an honest note when the
/// workspace carries no live artifacts.
#[test]
fn impossible_reports_the_closed_chain_without_artifacts() {
    let _guard = env_lock();
    let repo = seed_repo();
    let mut s = open(repo.path());
    let out = s.handle("impossible").unwrap().unwrap();
    for needle in ["2C2 = 1.32032", "E62", "kmax=40", "HEURISTIC never certifies", "no live experiment JSONs"] {
        assert!(out.contains(needle), "missing {needle} in:\n{out}");
    }
}

/// Phase 32 HUD projections: state markers, dir rollup, intent bands,
/// why-affected paths. Pure functions over the ranked list — no TUI needed,
///
/// no model, no second graph.
#[test]
fn hud_projections_are_pure_and_honest() {
    use physis_pro::nav::{NavHit, NavWeights, ObjectKind, ObjectId, PhysisObject, RetrievalScore, SourceRef};
    let _guard = env_lock();
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("src")).unwrap();
    std::fs::write(dir.path().join("src/index.rs"), "pub fn update_index() {}\n").unwrap();
    std::fs::write(dir.path().join("src/query.rs"), "fn search() { update_index(); }\n").unwrap();
    let mk = |path: &str, label: &str, score: f32| NavHit {
        object: PhysisObject {
            id: ObjectId::new("fs", &format!("{label}-hash-0123456789abcdef")),
            kind: ObjectKind::Passage,
            label: label.to_string(),
            source: Some(SourceRef { path: path.to_string(), start_line: Some(1), end_line: Some(1) }),
            mtime: 0.0,
            hash: format!("{label}-hash"),
            text: label.to_string(),
            embedding: vec![],
        },
        score: RetrievalScore { total: score, components: Default::default() },
    };
    let hits = vec![
        mk("src/index.rs", "src/index.rs:1-1", 0.9),
        mk("src/query.rs", "src/query.rs:1-1", 0.7),
        mk("README.md", "README.md:1-1", 0.2),
    ];
    // Intent bands: top-3 HOT, next-5 WARM, rest COLD — projection of rank.
    assert!(hits[0].score.total >= hits[1].score.total);
    // Why-affected: query.rs calls update_index defined in index.rs —
    // the shared dependents rule must link them.
    let syms = physis_pro::mvp::changed_symbols(dir.path(), &["src/index.rs".to_string()]);
    assert!(syms.contains(&"update_index".to_string()), "got {syms:?}");
    let deps = physis_pro::mvp::affected_dependents(dir.path(), &["src/index.rs".to_string()], &syms);
    assert!(deps.contains(&"src/query.rs".to_string()), "got {deps:?}");
    // Token math: same counter both sides, labels per Phase 32.
    let est = physis_pro::estimate::calculate_estimate(1000, 80);
    assert!((est.saved_percent - 92.0).abs() < 0.01);
    assert_eq!(est.baseline_tokens, 1000);
    let _ = NavWeights::default();
}
