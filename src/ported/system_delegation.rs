// provenance: moved from physis-core src/system_delegation.rs (core HEAD d65858b)
// as part of the Core/Product split, 2026-09-14. One implementation; no fork.
//! Delegation planning and unstarted worktree reservations over workspace observations.
//!
//! No worker is executed here. Requests, context, source fingerprints and literal
//! argv are reviewable before dispatch. The log owns task/lease state; folders
//! and JSON output are views. Reservations expire only because no process runs.

use crate::ported::system::{Workspace, WriteLock};
use anyhow::{ensure, Context, Result};
use chrono::{DateTime, Duration, Utc};
use physis_core::{
    observe,
    process::{ProcessTask, TaskState},
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    process::Command,
};

const SOURCE: &str = "system.delegation";
const SCHEMA: &str = "physis.task.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Read,
    Plan,
    Write,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Cline,
    Opencode,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cost {
    Local,
    FreeTier,
    Paid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    #[serde(default = "default_costs")]
    pub costs: Vec<Cost>,
    #[serde(default)]
    pub allow_remote_model: bool,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,
}
fn default_costs() -> Vec<Cost> {
    vec![Cost::Local, Cost::FreeTier]
}
fn default_timeout() -> u32 {
    900
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            costs: default_costs(),
            allow_remote_model: false,
            timeout_seconds: default_timeout(),
        }
    }
}
fn default_budget() -> usize {
    2400
}

/// Input excludes IDs: Physis assigns them when a task is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub intent: String,
    pub paths: Vec<PathBuf>,
    pub mode: Mode,
    pub acceptance: Vec<String>,
    #[serde(default)]
    pub checks: Vec<Vec<String>>,
    #[serde(default)]
    pub worker: Option<String>,
    #[serde(default = "default_budget")]
    pub budget: usize,
    #[serde(default)]
    pub policy: Policy,
}

/// Operator configuration; prices and locality are declarations, not measurements.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Worker {
    pub id: String,
    pub kind: Kind,
    pub program: PathBuf,
    pub provider: String,
    pub model: String,
    pub cost: Cost,
    pub remote_model: bool,
    pub modes: Vec<Mode>,
    pub max_parallel: usize,
    #[serde(default)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Registry {
    pub schema: String,
    pub workers: Vec<Worker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub head: String,
    pub status: String,
    pub fingerprint: String,
    pub scoped_files: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub schema: String,
    pub task: ProcessTask,
    pub correlation_id: String,
    pub request: Request,
    pub worker: Worker,
    pub source: Snapshot,
    pub context: Value,
    pub prior_work: Value,
    pub duplicates: Vec<String>,
    pub worktree: PathBuf,
    pub invocation: Vec<String>,
    pub notices: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reservation {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskView {
    pub plan: Plan,
    pub observation: String,
    pub reservation: Option<Reservation>,
}

pub fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let mut data = Vec::new();
    File::open(path)?.take(1_048_577).read_to_end(&mut data)?;
    ensure!(data.len() <= 1_048_576, "input exceeds 1 MiB");
    serde_json::from_slice(&data).with_context(|| format!("invalid JSON in {}", path.display()))
}

fn scoped_paths(ws: &Workspace, paths: &[PathBuf]) -> Result<()> {
    ensure!(
        !paths.is_empty() && paths.len() <= 64,
        "supply 1..64 scoped paths"
    );
    for path in paths {
        ensure!(
            !path.as_os_str().is_empty()
                && path.components().all(|c| matches!(c, Component::Normal(_))),
            "scope paths must be relative without . or .."
        );
        // The normal workspace inventory omits hidden/build directories. Do not
        // promise to fingerprint or pack files that this view cannot see.
        for part in path.components() {
            let name = part.as_os_str().to_string_lossy();
            ensure!(
                !name.starts_with('.') && !ws.skip_dirs.iter().any(|d| d == &name),
                "scope includes an excluded directory or hidden path: {}",
                path.display()
            );
        }
        let mut ancestor = ws.root.clone();
        for part in path.components() {
            ancestor.push(part);
            if let Ok(meta) = fs::symlink_metadata(&ancestor) {
                ensure!(
                    !meta.file_type().is_symlink(),
                    "scope cannot contain symlinks: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn git(ws: &Workspace, args: &[&str]) -> Result<Vec<u8>> {
    let out = Command::new("git")
        .arg("--no-optional-locks")
        .arg("-C")
        .arg(&ws.root)
        .args(args)
        .output()?;
    ensure!(
        out.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(out.stdout)
}

fn snapshot(ws: &Workspace, paths: &[PathBuf]) -> Result<Snapshot> {
    scoped_paths(ws, paths)?;
    let top = git(ws, &["rev-parse", "--show-toplevel"])?;
    ensure!(
        Path::new(String::from_utf8(top)?.trim()).canonicalize()? == ws.root,
        "--root must name the Git repository root; use the nested repository for submodule work"
    );
    let head = String::from_utf8(git(ws, &["rev-parse", "HEAD"])?)?
        .trim()
        .to_string();
    let exclude = ws
        .state
        .strip_prefix(&ws.root)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .map(|p| format!(":(literal,exclude){}", p.display()));
    let mut status_args = vec![
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--",
        ".",
    ];
    if let Some(exclude) = &exclude {
        status_args.push(exclude);
    }
    let status = git(ws, &status_args)?;
    let mut hasher = Sha256::new();
    hasher.update(head.as_bytes());
    hasher.update(&status);
    hasher.update(git(
        ws,
        &[
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--binary",
            "HEAD",
            "--",
        ],
    )?);
    let inventory = ws.inventory()?;
    ensure!(
        !inventory.truncated && inventory.errors.is_empty(),
        "source inventory incomplete; narrow the repository root"
    );
    let mut files = 0;
    for obj in inventory.objects {
        if !paths.iter().any(|p| Path::new(&obj.path).starts_with(p)) {
            continue;
        }
        // Refuse nested repositories: their dirty content is not represented
        // by the superproject HEAD. Plan against that repository directly.
        let absolute = ws.root.join(&obj.path);
        let mut parent = absolute.parent();
        while let Some(dir) = parent.filter(|p| *p != ws.root) {
            ensure!(
                !dir.join(".git").exists(),
                "scope crosses a nested repository; set --root to {}",
                dir.display()
            );
            parent = dir.parent();
        }
        hasher.update(obj.path.as_bytes());
        hasher.update([0]);
        let mut file = File::open(absolute)?;
        let mut buf = [0u8; 8192];
        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        hasher.update([0]);
        files += 1;
    }
    Ok(Snapshot {
        head,
        status: String::from_utf8_lossy(&status).replace('\0', "\n"),
        fingerprint: format!("{:x}", hasher.finalize()),
        scoped_files: files,
    })
}

fn loopback(url: &str) -> bool {
    let Some(authority) = url.strip_prefix("http://") else {
        return false;
    };
    let Some((host, port)) = authority.rsplit_once(':') else {
        return false;
    };
    matches!(host, "127.0.0.1" | "[::1]") && port.parse::<u16>().is_ok_and(|p| p > 0)
}

fn validate_worker(w: &Worker) -> Result<()> {
    ensure!(
        !w.id.is_empty() && !w.model.is_empty() && !w.provider.is_empty(),
        "worker id, provider and model must be explicit"
    );
    ensure!(
        (1..=64).contains(&w.max_parallel),
        "max_parallel must be 1..64"
    );
    ensure!(
        w.program.is_absolute(),
        "worker program must be an absolute path"
    );
    if let Some(server) = &w.server {
        ensure!(
            w.kind == Kind::Opencode && loopback(server),
            "serve requires an OpenCode loopback HTTP URL with explicit port"
        );
    }
    if w.kind == Kind::Opencode {
        ensure!(
            w.model.starts_with(&format!("{}/", w.provider)),
            "OpenCode model must be provider/model"
        );
    }
    Ok(())
}

fn executable(path: &Path) -> bool {
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn invocation(plan: &Plan) -> Vec<String> {
    let w = &plan.worker;
    let cwd = plan.worktree.display().to_string();
    let mut argv = vec![w.program.display().to_string()];
    match w.kind {
        Kind::Cline => {
            argv.extend([
                "--cwd".into(),
                cwd,
                "--json".into(),
                "--auto-approve".into(),
                "false".into(),
                "--timeout".into(),
                plan.request.policy.timeout_seconds.to_string(),
                "--provider".into(),
                w.provider.clone(),
                "--model".into(),
                w.model.clone(),
            ]);
            if plan.request.mode == Mode::Plan {
                argv.push("--plan".into());
            }
        }
        Kind::Opencode => {
            argv.extend([
                "run".into(),
                "--dir".into(),
                cwd,
                "--model".into(),
                w.model.clone(),
                "--format".into(),
                "json".into(),
            ]);
            if let Some(server) = &w.server {
                argv.extend(["--attach".into(), server.clone()]);
            }
        }
    }
    argv.extend(["--".into(), format!("Read the Physis task packet at .physis-task.json. Task {}. Intent: {}. Follow its scope and acceptance criteria; report evidence and changed paths.", plan.task.id, plan.request.intent)]);
    argv
}

/// Reconstruct all task/lease state, never a history tail that can forget a lease.
pub fn delegation_tasks(ws: &Workspace) -> Result<Vec<TaskView>> {
    let mut tasks = BTreeMap::<String, TaskView>::new();
    let log = match fs::read_to_string(ws.state.join("observations.jsonl")) {
        Ok(log) => log,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e.into()),
    };
    for line in log.lines().filter(|l| !l.trim().is_empty()) {
        // observe::read deliberately skips malformed lines for browsing. Lease
        // accounting must instead stop, or a torn reservation could disappear.
        let obs: observe::Observation = serde_json::from_str(line)
            .context("corrupt observation log; reservation state is unknown")?;
        if obs.source != SOURCE {
            continue;
        }
        let data: Value = serde_json::from_str(&obs.body).context("invalid delegation event")?;
        if data["workspace"].as_str() != ws.root.to_str() {
            continue;
        }
        let id = data["task_id"]
            .as_str()
            .context("delegation event has no task_id")?;
        match data["event"].as_str() {
            Some("created") => {
                let plan: Plan = serde_json::from_value(data["plan"].clone())?;
                ensure!(
                    plan.task.id == id && !tasks.contains_key(id),
                    "invalid or duplicated task identity"
                );
                tasks.insert(
                    id.to_owned(),
                    TaskView {
                        plan,
                        observation: format!("obs:{}", obs.seq),
                        reservation: None,
                    },
                );
            }
            Some("reserved") => {
                tasks
                    .get_mut(id)
                    .context("reservation without task")?
                    .reservation = Some(serde_json::from_value(data["reservation"].clone())?)
            }
            Some("released") => {
                tasks
                    .get_mut(id)
                    .context("release without task")?
                    .reservation = None
            }
            _ => anyhow::bail!("unsupported delegation event"),
        }
    }
    Ok(tasks.into_values().collect())
}

pub fn delegation_show(ws: &Workspace, id: &str) -> Result<TaskView> {
    delegation_tasks(ws)?
        .into_iter()
        .find(|t| t.plan.task.id == id)
        .context("task not found in this workspace")
}

/// Pure preview: Git reads, scoped Physis packing and recall; no directories or processes created.
pub fn delegation_plan(ws: &Workspace, request: &Request, registry: &Registry) -> Result<Plan> {
    ensure!(
        request.schema == SCHEMA && registry.schema == "physis.workers.v1",
        "unsupported schema version"
    );
    ensure!(
        !request.intent.trim().is_empty() && request.intent.len() <= 8192,
        "intent must be 1..8192 bytes"
    );
    ensure!(
        !request.acceptance.is_empty() && request.acceptance.iter().all(|s| !s.trim().is_empty()),
        "acceptance criteria are required"
    );
    ensure!(
        request
            .checks
            .iter()
            .all(|argv| !argv.is_empty() && !argv[0].is_empty()),
        "checks require explicit argv"
    );
    ensure!(
        (50..=20_000).contains(&request.budget),
        "budget must be 50..20000 Physis tokens"
    );
    ensure!(
        (1..=86400).contains(&request.policy.timeout_seconds),
        "timeout must be 1..86400 seconds"
    );
    ensure!(
        serde_json::to_vec(request)?.len() <= 64 * 1024,
        "task request exceeds 64 KiB"
    );
    let mut ids = std::collections::BTreeSet::new();
    for worker in &registry.workers {
        validate_worker(worker)?;
        ensure!(ids.insert(&worker.id), "duplicate worker id");
    }
    let tasks = delegation_tasks(ws)?;
    let now = Utc::now();
    let worker = registry.workers.iter().find(|w| {
        request.worker.as_ref().is_none_or(|id| id == &w.id)
            && request.policy.costs.contains(&w.cost)
            && (!w.remote_model || request.policy.allow_remote_model)
            && w.modes.contains(&request.mode) && executable(&w.program)
            && tasks.iter().filter(|t| t.plan.worker.id == w.id && t.reservation.as_ref().is_some_and(|r| r.expires_at > now)).count() < w.max_parallel
    }).context("no eligible worker: check explicit model, cost/remote policy, executable, mode and reservation capacity")?.clone();
    let source = snapshot(ws, &request.paths)?;
    let context = ws.pack_paths(&request.intent, request.budget, &request.paths)?;
    ensure!(
        snapshot(ws, &request.paths)? == source,
        "source changed while planning; retry against stable files"
    );
    let digest = format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(&(request, &worker, &source))?)
    );
    let id = format!("preview-{}", &digest[..24]);
    let duplicates = tasks
        .iter()
        .filter(|t| {
            t.plan.request.intent == request.intent && t.plan.request.paths == request.paths
        })
        .map(|t| t.plan.task.id.clone())
        .collect();
    let mut plan = Plan {
        schema: "physis.delegation-plan.v1".into(),
        task: ProcessTask { id: id.clone(), title: request.intent.clone(), state: TaskState::Pending, assigned_resources: vec![worker.id.clone()], dependencies: vec![], expected_duration_secs: Some(request.policy.timeout_seconds.into()) },
        correlation_id: id.clone(), request: request.clone(), worker, source, context,
        prior_work: ws.history(Some(&request.intent), 5)?, duplicates,
        worktree: ws.state.join("delegation/worktrees").join(&id), invocation: vec![],
        notices: vec![
            "Preview only: no worker, worktree, model request, or acceptance check has run.".into(),
            "Cost and model locality are registry declarations; availability and pricing have not been probed.".into(),
            "Paths bound context selection; worker filesystem/network restrictions are not enforced by this planner. A worktree is not a sandbox.".into(),
            "Before dispatch: create an isolated worktree with the recorded source changes, materialize .physis-task.json, and enforce timeout and permissions.".into(),
            "Git status covers the repository; the fingerprint includes tracked diffs and visible scoped file contents. Hidden/ignored files are not copied into a future worktree.".into(),
        ],
    };
    plan.invocation = invocation(&plan);
    Ok(plan)
}

fn lock(ws: &Workspace) -> Result<WriteLock> {
    fs::create_dir_all(&ws.state)?;
    WriteLock::acquire(ws.state.join("delegation-write.lock"))
}

/// Persist one task in the existing observation log; duplicates remain explicit.
pub fn delegation_create(
    ws: &Workspace,
    request: &Request,
    registry: &Registry,
) -> Result<TaskView> {
    let mut plan = delegation_plan(ws, request, registry)?;
    let _guard = lock(ws)?;
    ensure!(
        snapshot(ws, &request.paths)? == plan.source,
        "source changed before task creation"
    );
    plan.duplicates = delegation_tasks(ws)?
        .iter()
        .filter(|t| {
            t.plan.request.intent == request.intent && t.plan.request.paths == request.paths
        })
        .map(|t| t.plan.task.id.clone())
        .collect();
    let id = format!("task-{}", uuid::Uuid::new_v4());
    plan.task.id = id.clone();
    plan.correlation_id = id.clone();
    plan.worktree = ws.state.join("delegation/worktrees").join(&id);
    plan.invocation = invocation(&plan);
    let obs = ws.record(
        SOURCE,
        &format!("created {id}: {}", request.intent),
        json!({"event":"created", "task_id":id, "plan":plan, "outcome":"unverified"}),
    )?;
    Ok(TaskView {
        plan,
        observation: format!("obs:{}", obs.seq),
        reservation: None,
    })
}

/// Serialize competing reservations. No worktree or worker is started.
pub fn delegation_reserve(ws: &Workspace, id: &str, seconds: u32) -> Result<Reservation> {
    ensure!(
        (1..=86400).contains(&seconds),
        "lease duration must be 1..86400 seconds"
    );
    let _guard = lock(ws)?;
    let tasks = delegation_tasks(ws)?;
    let task = tasks
        .iter()
        .find(|t| t.plan.task.id == id)
        .context("task not found")?;
    let now = Utc::now();
    ensure!(
        !task
            .reservation
            .as_ref()
            .is_some_and(|r| r.expires_at > now),
        "task already reserved"
    );
    ensure!(
        snapshot(ws, &task.plan.request.paths)? == task.plan.source,
        "source changed since task creation; create a fresh task"
    );
    ensure!(
        !task.plan.worktree.exists(),
        "proposed worktree already exists; inspect before reserving"
    );
    let active: Vec<_> = tasks
        .iter()
        .filter(|t| t.reservation.as_ref().is_some_and(|r| r.expires_at > now))
        .collect();
    ensure!(
        !active.iter().any(|t| t.plan.worktree == task.plan.worktree),
        "worktree already reserved"
    );
    let capacity = tasks
        .iter()
        .filter(|t| {
            t.plan.worker.id == task.plan.worker.id
                && (t.plan.task.id == id
                    || t.reservation.as_ref().is_some_and(|r| r.expires_at > now))
        })
        .map(|t| t.plan.worker.max_parallel)
        .min()
        .unwrap_or(1);
    ensure!(
        active
            .iter()
            .filter(|t| t.plan.worker.id == task.plan.worker.id)
            .count()
            < capacity,
        "worker reservation capacity reached"
    );
    let reservation = Reservation {
        token: uuid::Uuid::new_v4().to_string(),
        expires_at: now + Duration::seconds(seconds.into()),
    };
    ws.record(SOURCE, &format!("reserved {id}"), json!({"event":"reserved", "task_id":id, "reservation":reservation, "outcome":"unverified"}))?;
    Ok(reservation)
}

pub fn delegation_release(ws: &Workspace, id: &str, token: &str) -> Result<Value> {
    let _guard = lock(ws)?;
    let task = delegation_show(ws, id)?;
    let reservation = task.reservation.context("task has no reservation")?;
    ensure!(
        reservation.token == token,
        "reservation token does not match current owner"
    );
    let obs = ws.record(
        SOURCE,
        &format!("released {id}"),
        json!({"event":"released", "task_id":id, "token":token, "outcome":"unverified"}),
    )?;
    Ok(json!({"task_id":id, "released":true, "observation":format!("obs:{}", obs.seq)}))
}

/// A portable folder view, derived from the same task ID and observation.
pub fn delegation_export(ws: &Workspace, id: &str, destination: &Path) -> Result<Value> {
    let task = delegation_show(ws, id)?;
    fs::create_dir(destination).context("export requires a new directory")?;
    let path = destination.join("task.json");
    let mut out = File::options().write(true).create_new(true).open(&path)?;
    out.write_all(&serde_json::to_vec_pretty(&task)?)?;
    out.sync_all()?;
    Ok(json!({"task_id":id,"observation":task.observation,"path":path,"snapshot":true}))
}
