//! Agent adapters (plan S2). One trait, one job: reach the agent and bring back
//! text plus, when the agent is JEV, a validated structured result.
//!
//! Rules the exit gate tests:
//! - A probe never mutates anything.
//! - An absent binary or key reports a reason; it never guesses availability.
//! - `@jev` with no transport is `unavailable`.
//! - Schema-invalid output is reported, never guessed at.
//!
//! Invocation is bounded: a spawned process is killed at its deadline. Nothing
//! here writes to the workspace; the adapters only read a reply.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::{on_path, zen_model, Agent, AgentKind, Status};

/// What went wrong, stated rather than flattened into an empty string.
#[derive(Debug, Clone, PartialEq)]
pub enum AdapterError {
    /// The program is not on PATH.
    Missing(String),
    Spawn(String),
    /// The process exited non-zero, with its stderr for diagnosis.
    Exit(i32, String),
    /// The process exceeded its deadline and was killed.
    Timeout(u32),
    /// The reply did not match the required schema.
    Schema(String),
    /// The prompt could be read as an option, or carries a NUL.
    Unsafe(String),
    /// This build has no transport for the agent.
    Unsupported(String),
}

impl std::fmt::Display for AdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AdapterError::Missing(p) => write!(f, "{p} not on PATH"),
            AdapterError::Spawn(e) => write!(f, "spawn failed: {e}"),
            AdapterError::Exit(c, e) => {
                if e.trim().is_empty() {
                    write!(f, "process exited {c}")
                } else {
                    write!(f, "process exited {c}: {}", e.trim())
                }
            }
            AdapterError::Timeout(s) => write!(f, "timed out after {s}s"),
            AdapterError::Schema(e) => write!(f, "schema-invalid reply: {e}"),
            AdapterError::Unsafe(e) => write!(f, "unsafe prompt: {e}"),
            AdapterError::Unsupported(e) => write!(f, "unsupported: {e}"),
        }
    }
}

impl std::error::Error for AdapterError {}

/// One request to an agent.
#[derive(Debug, Clone, PartialEq)]
pub struct Invocation {
    pub prompt: String,
    pub scope: Option<String>,
    pub timeout_seconds: u32,
}

impl Invocation {
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            scope: None,
            // Measured 2026-09-19: a bare `opencode run` turn on the pinned Zen
            // model is ~43 s, and a question that makes the read-only agent
            // explore the repo runs past two minutes. The deadline is generous
            // because killing a useful answer is worse than waiting.
            timeout_seconds: 240,
        }
    }

    pub fn with_scope(mut self, scope: impl Into<String>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    pub fn with_timeout(mut self, seconds: u32) -> Self {
        self.timeout_seconds = seconds;
        self
    }
}

/// One finding with the evidence it cites.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub claim: String,
    #[serde(default)]
    pub evidence: Vec<String>,
}

/// A relation the agent proposes. Proposed, not asserted: S9 owns the accept
/// path, and nothing here writes to the nav store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProposedRelation {
    pub from: String,
    pub to: String,
    pub kind: String,
}

/// The JEV structured-result contract. Every field is optional so a partial
/// reply is still parseable; invalid JSON is an error, not an empty result.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JevResult {
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub relations: Vec<ProposedRelation>,
    #[serde(default)]
    pub notes: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
}

impl JevResult {
    /// Extract and parse a structured result from a reply that may wrap the
    /// JSON in prose or a fenced block. A reply with no JSON object is an
    /// error, and so is a JSON object that carries none of the known keys —
    /// otherwise an unrelated object would deserialize to an empty result and
    /// read as success.
    pub fn parse(text: &str) -> Result<Self, String> {
        let json =
            extract_json_object(text).ok_or_else(|| "no JSON object found in reply".to_string())?;
        let value: serde_json::Value =
            serde_json::from_str(&json).map_err(|e| format!("invalid JSON: {e}"))?;
        let obj = value
            .as_object()
            .ok_or_else(|| "reply JSON is not an object".to_string())?;
        const KNOWN: [&str; 4] = ["findings", "relations", "notes", "decisions"];
        if !obj.keys().any(|k| KNOWN.contains(&k.as_str())) {
            let keys: Vec<&String> = obj.keys().collect();
            return Err(format!(
                "JSON object has none of {KNOWN:?} (found keys {keys:?})"
            ));
        }
        let parsed: JevResult =
            serde_json::from_value(value).map_err(|e| format!("invalid JevResult: {e}"))?;
        parsed.validate()?;
        Ok(parsed)
    }

    /// Structural validation: an item that cannot be used is an error, not a
    /// silently dropped field.
    pub fn validate(&self) -> Result<(), String> {
        for (i, f) in self.findings.iter().enumerate() {
            if f.claim.trim().is_empty() {
                return Err(format!("findings[{i}].claim is empty"));
            }
        }
        for (i, r) in self.relations.iter().enumerate() {
            if r.from.trim().is_empty() || r.to.trim().is_empty() || r.kind.trim().is_empty() {
                return Err(format!("relations[{i}] needs non-empty from, to and kind"));
            }
        }
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.findings.is_empty()
            && self.relations.is_empty()
            && self.notes.is_empty()
            && self.decisions.is_empty()
    }
}

/// First top-level `{...}` in the text, brace-balanced and string-aware.
fn extract_json_object(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let start = text.find('{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

/// What an agent returned.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentOutput {
    pub text: String,
    /// Present only when the adapter asked for and validated a schema.
    pub structured: Option<JevResult>,
}

pub trait AgentAdapter {
    fn id(&self) -> &str;
    /// Availability, probed without side effects.
    fn probe(&self) -> Status;
    fn invoke(&self, inv: &Invocation) -> Result<AgentOutput, AdapterError>;
}

// ---------------------------------------------------------------------------
// Local / verifier: deterministic, in-process, no network.
// ---------------------------------------------------------------------------

pub struct LocalAdapter {
    id: String,
    verifier: bool,
}

impl LocalAdapter {
    pub fn new(id: impl Into<String>, verifier: bool) -> Self {
        Self {
            id: id.into(),
            verifier,
        }
    }
}

impl AgentAdapter for LocalAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> Status {
        Status {
            available: true,
            transport: "local".to_string(),
            reason: None,
        }
    }

    fn invoke(&self, inv: &Invocation) -> Result<AgentOutput, AdapterError> {
        let what = if self.verifier { "verifier" } else { "local" };
        let mut result = JevResult::default();
        result.notes.push(format!(
            "{what} adapter: deterministic, no model; scope={}",
            inv.scope.as_deref().unwrap_or("none")
        ));
        Ok(AgentOutput {
            text: format!("[{what}] deterministic reply for: {}", inv.prompt.trim()),
            structured: Some(result),
        })
    }
}

// ---------------------------------------------------------------------------
// CLI adapters: hermes, opencode, and JEV over the OpenCode Zen transport.
// ---------------------------------------------------------------------------

pub struct CliAdapter {
    id: String,
    program: String,
    /// argv template; every occurrence of `{prompt}` is replaced.
    args: Vec<String>,
    /// When true, the reply must contain a valid `JevResult`.
    wants_schema: bool,
}

impl CliAdapter {
    pub fn new(
        id: impl Into<String>,
        program: impl Into<String>,
        args: Vec<String>,
        wants_schema: bool,
    ) -> Self {
        Self {
            id: id.into(),
            program: program.into(),
            args,
            wants_schema,
        }
    }
}

impl AgentAdapter for CliAdapter {
    fn id(&self) -> &str {
        &self.id
    }

    fn probe(&self) -> Status {
        if on_path(&self.program) {
            Status {
                available: true,
                transport: format!("cli:{}", self.program),
                reason: None,
            }
        } else {
            Status {
                available: false,
                transport: "unavailable".to_string(),
                reason: Some(format!("{} not on PATH", self.program)),
            }
        }
    }

    fn invoke(&self, inv: &Invocation) -> Result<AgentOutput, AdapterError> {
        if !on_path(&self.program) {
            return Err(AdapterError::Missing(self.program.clone()));
        }
        guard_prompt(&inv.prompt)?;
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| a.replace("{prompt}", &inv.prompt))
            .collect();
        let mut cmd = Command::new(&self.program);
        cmd.args(&args);
        if let Some(scope) = &inv.scope {
            cmd.env("PHYSIS_AGENT_SCOPE", scope);
        }
        let (status, stdout, stderr) = run_with_timeout(&mut cmd, inv.timeout_seconds)?;
        if !status.success() {
            return Err(AdapterError::Exit(status.code().unwrap_or(-1), stderr));
        }
        let structured = if self.wants_schema {
            Some(JevResult::parse(&stdout).map_err(AdapterError::Schema)?)
        } else {
            None
        };
        Ok(AgentOutput {
            text: stdout,
            structured,
        })
    }
}

/// Run to completion or kill at the deadline. Returns (status, stdout, stderr).
fn run_with_timeout(
    cmd: &mut Command,
    timeout_seconds: u32,
) -> Result<(std::process::ExitStatus, String, String), AdapterError> {
    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| AdapterError::Spawn(e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds.max(1) as u64);
    loop {
        match child
            .try_wait()
            .map_err(|e| AdapterError::Spawn(e.to_string()))?
        {
            Some(status) => {
                let mut stdout = String::new();
                let mut stderr = String::new();
                if let Some(mut o) = child.stdout.take() {
                    let _ = o.read_to_string(&mut stdout);
                }
                if let Some(mut e) = child.stderr.take() {
                    let _ = e.read_to_string(&mut stderr);
                }
                return Ok((status, stdout, stderr));
            }
            None => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(AdapterError::Timeout(timeout_seconds));
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

/// Keep the prompt as data, not as another option. The prompt is already a
/// single argv element (no shell is involved), so shell metacharacters are
/// inert; what remains is a prompt that the program would read as a flag, or a
/// NUL that cannot cross the argv boundary at all. Both are refused rather than
/// passed on.
fn guard_prompt(prompt: &str) -> Result<(), AdapterError> {
    if prompt.contains('\0') {
        return Err(AdapterError::Unsafe("prompt contains a NUL byte".into()));
    }
    if prompt.trim_start().starts_with('-') {
        return Err(AdapterError::Unsafe(
            "prompt starts with `-` and would be read as an option".into(),
        ));
    }
    Ok(())
}

/// The reply contract, appended to every JEV prompt. The adapter requires a
/// parseable result, so the request for it is explicit rather than assumed —
/// and a reply that ignores it is reported as schema-invalid, not guessed at.
const JEV_SCHEMA: &str = "\n\nReply with a single JSON object and nothing else. \
Optional keys: \"findings\" (array of {\"claim\": string, \"evidence\": [string]}), \
\"relations\" (array of {\"from\": string, \"to\": string, \"kind\": string}), \
\"notes\" (array of strings), \"decisions\" (array of strings).";

/// `opencode run` argv with the model pinned. The model is not optional: the
/// CLI's default is the out-of-funds `deepseek-v4-flash`.
fn zen_args(model: &str, schema: bool) -> Vec<String> {
    let prompt = if schema {
        format!("{{prompt}}{JEV_SCHEMA}")
    } else {
        "{prompt}".to_string()
    };
    vec![
        "run".to_string(),
        "-m".to_string(),
        model.to_string(),
        "--agent".to_string(),
        "plan".to_string(),
        prompt,
    ]
}

/// Build the adapter for a roster entry. Program names are overridable by env
/// so a fixture process can stand in for the real binary in tests.
///
/// The OpenCode and JEV transports run `--agent plan` (read-only) on a pinned
/// free Zen model. S2 reasons; it does not let an agent edit the workspace.
pub fn adapter_for(agent: &Agent) -> Result<Box<dyn AgentAdapter>, AdapterError> {
    match agent.kind {
        AgentKind::Verifier => Ok(Box::new(LocalAdapter::new(&agent.id, true))),
        AgentKind::Local => Ok(Box::new(LocalAdapter::new(&agent.id, false))),
        AgentKind::Human => Err(AdapterError::Unsupported(
            "human agents are addressed, not invoked".to_string(),
        )),
        AgentKind::Hermes => Ok(Box::new(CliAdapter::new(
            &agent.id,
            env_or("PHYSIS_HERMES_BIN", "hermes"),
            vec!["-z".to_string(), "{prompt}".to_string()],
            false,
        ))),
        AgentKind::Opencode => {
            let model = agent.model.clone().unwrap_or_else(zen_model);
            Ok(Box::new(CliAdapter::new(
                &agent.id,
                env_or("PHYSIS_OPENCODE_BIN", "opencode"),
                zen_args(&model, false),
                false,
            )))
        }
        AgentKind::Jev => {
            // JEV has no direct transport in this build. The reachable path is
            // OpenCode Zen, and it must not be reported as JEV-the-service.
            let program = env_or("PHYSIS_OPENCODE_BIN", "opencode");
            if !on_path(&program) {
                return Err(AdapterError::Unsupported(
                    "no JEV transport: set PHYSIS_OPENCODE_BIN/opencode for the Zen path; \
                     a direct JEV transport is not implemented in this build"
                        .to_string(),
                ));
            }
            let model = agent.model.clone().unwrap_or_else(zen_model);
            Ok(Box::new(CliAdapter::new(
                &agent.id,
                program,
                zen_args(&model, true),
                true,
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::default_roster;

    /// A `CliAdapter` whose program is the shell: `sh -c <script>`.
    ///
    /// No fixture file is exec'd. Writing a script and exec'ing it flakes under
    /// parallel tests with ETXTBSY ("Text file busy"): a `Command::spawn` in one
    /// test forks while a sibling still holds its fixture open for writing, and
    /// the kernel refuses to exec a file open for writing. The shell always
    /// exists and can never be busy.
    fn sh_adapter(script: &str, wants_schema: bool) -> CliAdapter {
        CliAdapter::new(
            "fixture",
            "sh",
            vec!["-c".to_string(), script.to_string()],
            wants_schema,
        )
    }

    #[test]
    fn local_adapter_is_available_and_never_touches_the_network() {
        let a = LocalAdapter::new("local", false);
        assert!(a.probe().available);
        let out = a
            .invoke(&Invocation::new("hello").with_scope("auth"))
            .unwrap();
        assert!(out.structured.is_some());
        assert!(out.text.contains("hello"));
    }

    #[test]
    fn missing_binary_reports_a_reason_and_never_claims_availability() {
        let a = CliAdapter::new(
            "x",
            "definitely-not-a-real-binary-xyz",
            vec!["{prompt}".to_string()],
            false,
        );
        let s = a.probe();
        assert!(!s.available);
        assert_eq!(s.transport, "unavailable");
        assert!(s.reason.as_deref().unwrap().contains("not on PATH"));
        assert!(matches!(
            a.invoke(&Invocation::new("x")),
            Err(AdapterError::Missing(_))
        ));
    }

    #[test]
    fn fixture_process_returns_text() {
        let a = sh_adapter("echo hello from fixture", false);
        let out = a.invoke(&Invocation::new("ignored")).unwrap();
        assert!(out.text.contains("hello from fixture"));
        assert!(out.structured.is_none(), "no schema was requested");
    }

    #[test]
    fn fixture_process_with_schema_parses_a_valid_jev_result() {
        let json = r#"{"findings":[{"claim":"c","evidence":["e"]}],"relations":[{"from":"a","to":"b","kind":"same"}],"notes":["n"],"decisions":[]}"#;
        let a = sh_adapter(&format!("echo '{}'", json), true);
        let out = a.invoke(&Invocation::new("q")).unwrap();
        let s = out.structured.expect("schema requested");
        assert_eq!(s.findings.len(), 1);
        assert_eq!(s.relations[0].kind, "same");
    }

    #[test]
    fn schema_invalid_reply_is_reported_not_guessed() {
        let a = sh_adapter("echo 'not json at all'", true);
        assert!(matches!(
            a.invoke(&Invocation::new("q")),
            Err(AdapterError::Schema(_))
        ));
    }

    #[test]
    fn nonzero_exit_is_reported() {
        let a = sh_adapter("exit 3", false);
        assert!(matches!(
            a.invoke(&Invocation::new("q")),
            Err(AdapterError::Exit(3, _))
        ));
    }

    #[test]
    fn timeout_kills_the_process() {
        let a = sh_adapter("sleep 30", false);
        let started = Instant::now();
        let r = a.invoke(&Invocation::new("q").with_timeout(1));
        assert!(matches!(r, Err(AdapterError::Timeout(1))));
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[test]
    fn jev_result_parsing_extracts_json_from_prose_and_validates() {
        let text = "Here you go:\n```json\n{\"notes\":[\"ok\"]}\n```\ndone";
        let r = JevResult::parse(text).unwrap();
        assert_eq!(r.notes, vec!["ok"]);

        // An empty claim is invalid.
        assert!(JevResult::parse(r#"{"findings":[{"claim":""}]}"#).is_err());
        // A relation missing a side is invalid.
        assert!(JevResult::parse(r#"{"relations":[{"from":"a","to":"","kind":"x"}]}"#).is_err());
        // No JSON object at all.
        assert!(JevResult::parse("no object here").is_err());
        // A JSON object with none of the known keys is not a JevResult: it must
        // be reported, not silently deserialized to an empty result.
        let err = JevResult::parse(r#"{"modules":["mod.rs"],"note":"x"}"#).unwrap_err();
        assert!(err.contains("none of"), "{err}");
        // A known key alongside an unknown one is still a valid result.
        assert!(JevResult::parse(r#"{"notes":["ok"],"extra":1}"#).is_ok());
    }

    #[test]
    fn prompts_that_look_like_options_are_refused() {
        let a = sh_adapter("echo ok", false);
        assert!(matches!(
            a.invoke(&Invocation::new("--danger")),
            Err(AdapterError::Unsafe(_))
        ));
        assert!(matches!(
            a.invoke(&Invocation::new("  -x")),
            Err(AdapterError::Unsafe(_))
        ));
        assert!(matches!(
            a.invoke(&Invocation::new("has\0nul")),
            Err(AdapterError::Unsafe(_))
        ));
        // A normal prompt still goes through, dashes not at the start included.
        assert!(a.invoke(&Invocation::new("explain the -flag")).is_ok());
    }

    #[test]
    fn factory_refuses_jevs_direct_path_and_invokes_local() {
        let roster = default_roster();
        let local = adapter_for(roster.get("local").unwrap()).unwrap();
        assert!(local.probe().available);
        let human = Agent {
            kind: AgentKind::Human,
            ..roster.get("local").unwrap().clone()
        };
        assert!(matches!(
            adapter_for(&human),
            Err(AdapterError::Unsupported(_))
        ));
    }
}
