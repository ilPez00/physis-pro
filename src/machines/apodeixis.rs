//! apodeixis — the demonstrative-proof machine. (Aristotelian *apódeixis* —
//! syllogistic demonstration, vs. elenchus' cross-examination. Together they
//! are the two faces of inquiry: elenchus exposes what is *unearned*, apodeixis
//! settles what is *provable*.)
//!
//! Perceive an obligation — a natural-language claim plus a Lean 4 formalization
//! (theorem + proof attempt); reduce it by checking it against the **Lean
//! kernel**; elaborate into a verdict: `certified` (the kernel accepted the
//! proof), `refuted` (the kernel rejected it — the first diagnostic is the
//! witness), or `untestable` (no Lean executable, the check timed out, or the
//! file could not be written/read).
//!
//! This is the hard half of the coherence engine: where elenchus scores a claim
//! "supported" by cosine similarity, apodeixis *decides* it by a machine-checked
//! proof — no room for hallucination in the verdict. Refuted obligations feed
//! back into the quality loop as failures to penalize and learn from.
//!
//! The generator of `snippet`s is deliberately out of scope: a proof-engine
//! model (Leanstral, Lean Copilot, so on) drafts the Lean declaration; apodeixis
//! is the verifier that decides. Shells to the system `lean` binary (`LEAN_BIN`
//! overrides PATH lookup), guarded by coreutils `timeout` so a bad proof cannot
//! hang the engine. Pure classifier + testable; the shell seam is a thin shim.

use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The Lean kernel accepted the theorem + proof.
    Certified,
    /// The kernel rejected it — the first error diagnostic is the witness.
    Refuted,
    /// Could not check (no Lean, timeout, IO error) — neither admitted nor beaten.
    Untestable,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Certified => "certified",
            Verdict::Refuted => "refuted",
            Verdict::Untestable => "untestable",
        }
    }

    /// A certified proof is the strongest possible coherence; a refutation is
    /// the strongest failure; untestable is neutral (no evidence either way).
    pub fn coherence(self) -> f32 {
        match self {
            Verdict::Certified => 1.0,
            Verdict::Refuted => -1.0,
            Verdict::Untestable => 0.0,
        }
    }
}

/// One proof obligation: the claim plus its Lean 4 formalization attempt.
#[derive(Debug, Clone, Deserialize)]
pub struct Obligation {
    pub id: String,
    #[serde(default)]
    pub claim: String,
    /// The full `.lean` file body: definitions + `theorem … := …` + `by …`.
    pub snippet: String,
}

/// The verdict for one obligation, with the kernel's evidence.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub id: String,
    #[serde(default)]
    pub claim: String,
    pub verdict: Verdict,
    /// First diagnostic line — the witness for a refutation, or why it couldn't
    /// be tested.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub witness: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    /// Which verifier produced the verdict ("lean", "unavailable").
    pub engine: String,
}

/// Pure, testable classifier: map an exit code + output onto a verdict.
/// - `Some(0)`  → Certified.
/// - `Some(_)`  → Refuted; `output` supplies the first error line.
/// - `None`     → Untestable (timeout/killed/no response); `output` is the reason.
pub fn classify(exit_code: Option<i32>, output: &str, engine: &str) -> (Verdict, String) {
    let witness = first_error(output);
    match exit_code {
        Some(0) => (Verdict::Certified, String::new()),
        Some(_) => (
            Verdict::Refuted,
            if witness.is_empty() { truncate(output) } else { witness },
        ),
        None => (
            Verdict::Untestable,
            if witness.is_empty() {
                if output.trim().is_empty() {
                    format!("verifier '{engine}' did not respond (timeout or killed)")
                } else {
                    truncate(output)
                }
            } else {
                witness
            },
        ),
    }
}

/// Parse "exit code + output" for every obligation. Pure — no shell.
pub fn evaluate(checks: &[Check]) -> VerdictReport {
    let mut certified = Vec::new();
    let mut refuted = Vec::new();
    let mut untestable = Vec::new();
    for c in checks {
        match c.verdict {
            Verdict::Certified => certified.push(c.id.clone()),
            Verdict::Refuted => refuted.push(c.id.clone()),
            Verdict::Untestable => untestable.push(c.id.clone()),
        }
    }
    VerdictReport { certified, refuted, untestable }
}

#[derive(Debug, Clone, Serialize)]
pub struct VerdictReport {
    #[serde(default)]
    pub certified: Vec<String>,
    #[serde(default)]
    pub refuted: Vec<String>,
    #[serde(default)]
    pub untestable: Vec<String>,
}

/// Check every obligation against the Lean kernel (serial — a verifier run holds
/// the CPU for the timeout budget). Missing Lean → every one is untestable, so a
/// machine that isn't installed reports the situation instead of erroring the
/// request.
pub fn reduce(obligations: &[Obligation], timeout_secs: u64) -> Vec<Check> {
    obligations.iter().map(|o| verify(o, timeout_secs)).collect()
}

fn lean_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("LEAN_BIN") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    for dir in std::env::split_paths(&std::env::var_os("PATH")?) {
        let cand = dir.join("lean");
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

/// Shell seam: write the snippet, run `timeout <n> <lean> <file>`, clean up.
/// Every failure mode collapses into Untestable with a reason witness.
pub fn verify(ob: &Obligation, timeout_secs: u64) -> Check {
    let msg = |verdict: Verdict, witness: String, exit: Option<i32>, engine: &str| Check {
        id: ob.id.clone(),
        claim: ob.claim.clone(),
        verdict,
        witness: truncate(&witness),
        exit_code: exit,
        engine: engine.to_string(),
    };

    let Some(lean) = lean_exe() else {
        return msg(
            Verdict::Untestable,
            "Lean 4 executable not found — install it (elan) or set LEAN_BIN".into(),
            None,
            "unavailable",
        );
    };

    let file = std::env::temp_dir().join(format!("apodeixis-{}.lean", uuid::Uuid::new_v4().simple()));
    if let Err(e) = std::fs::write(&file, &ob.snippet) {
        return msg(Verdict::Untestable, format!("could not write proof file: {e}"), None, "lean");
    }

    let output = Command::new("timeout")
        .arg(timeout_secs.to_string())
        .arg(&lean)
        .arg(&file)
        .output()
        // No coreutils `timeout`? Fall back to a direct run (no time budget).
        .or_else(|_| Command::new(&lean).arg(&file).output());

    let _ = std::fs::remove_file(&file);

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let stderr = String::from_utf8_lossy(&out.stderr);
            let (verdict, witness) = classify(out.status.code(), &format!("{stdout}\n{stderr}"), "lean");
            msg(verdict, witness, out.status.code(), "lean")
        }
        Err(e) => msg(Verdict::Untestable, format!("could not run Lean: {e}"), None, "lean"),
    }
}

/// Smallest first diagnostic: first line that mentions an error, stripped of
/// ANSI codes and truncated.
fn first_error(output: &str) -> String {
    let stripped = strip_ansi(output);
    let line = stripped
        .lines()
        .find(|l| l.contains("error"))
        .unwrap_or_else(|| stripped.lines().next().unwrap_or(""));
    truncate(line)
}

/// Kill ANSI escape sequences and control chaff so kernel output renders cleanly.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_esc = false;
    for c in s.chars() {
        if in_esc {
            if c == 'm' {
                in_esc = false;
            }
            continue;
        }
        if c == '\x1b' {
            in_esc = true;
            continue;
        }
        out.push(c);
    }
    out
}

fn truncate(s: &str) -> String {
    let t: String = s.trim().chars().take(260).collect();
    if t.chars().count() >= 260 {
        format!("{t}…")
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_zero_certifies() {
        let (v, w) = classify(Some(0), "successes\n", "lean");
        assert_eq!(v, Verdict::Certified);
        assert!(w.is_empty());
        assert_eq!(v.coherence(), 1.0);
    }

    #[test]
    fn non_zero_refutes_with_the_first_error() {
        let out = "\x1b[0m\x1b[31mfile.lean:2:0: error: type mismatch\n  x\nhas type\n  Nat\nbut expected\n  Bool\n\x1b[0mmore";
        let (v, w) = classify(Some(1), out, "lean");
        assert_eq!(v, Verdict::Refuted);
        assert!(w.contains("type mismatch"), "witness must name the failing step: {w}");
        assert!(w.len() < 300);
        assert_eq!(v.coherence(), -1.0);
    }

    #[test]
    fn no_response_is_untestable() {
        let (v, w) = classify(None, "", "lean");
        assert_eq!(v, Verdict::Untestable);
        assert!(w.contains("timeout"), "killed/absent → explain why: {w}");
        assert_eq!(v.coherence(), 0.0);
    }

    #[test]
    fn evaluate_groups_ids_by_verdict() {
        let mk = |id: &str, v: Verdict| Check {
            id: id.into(),
            claim: String::new(),
            verdict: v,
            witness: String::new(),
            exit_code: None,
            engine: "lean".into(),
        };
        let checks = vec![mk("a", Verdict::Certified), mk("b", Verdict::Refuted), mk("c", Verdict::Untestable)];
        let r = evaluate(&checks);
        assert_eq!(r.certified, vec!["a"]);
        assert_eq!(r.refuted, vec!["b"]);
        assert_eq!(r.untestable, vec!["c"]);
    }

    #[test]
    fn missing_lean_is_untestable_and_never_panics() {
        let ob = Obligation { id: "o".into(), claim: "x".into(), snippet: "#check True".into() };
        std::env::set_var("LEAN_BIN", "/nonexistent/lean");
        let c = verify(&ob, 5);
        assert_eq!(c.verdict, Verdict::Untestable);
        std::env::remove_var("LEAN_BIN");
    }

    #[test]
    fn real_lean_certifies_trivial_theorem() {
        let lean = std::env::var("LEAN_BIN").unwrap_or_else(|_| "lean".into());
        if std::process::Command::new(&lean).arg("--version").output().is_err() {
            eprintln!("skipping: lean not available");
            return;
        }
        let ob = Obligation {
            id: "real-trivial".into(),
            claim: "True is provable".into(),
            snippet: "#check True\n".into(),
        };
        let c = verify(&ob, 20);
        assert_eq!(c.verdict, Verdict::Certified, "expected Certified, got {:?} witness={}", c.verdict, c.witness);
        assert_eq!(c.engine, "lean");
    }

    #[test]
    fn real_lean_refutes_type_mismatch() {
        let lean = std::env::var("LEAN_BIN").unwrap_or_else(|_| "lean".into());
        if std::process::Command::new(&lean).arg("--version").output().is_err() {
            eprintln!("skipping: lean not available");
            return;
        }
        let ob = Obligation {
            id: "real-mismatch".into(),
            claim: "Nat is Bool".into(),
            snippet: r#"
def f (x : Nat) : Bool := x
theorem t : True := trivial
"#.into(),
        };
        let c = verify(&ob, 20);
        assert_eq!(c.verdict, Verdict::Refuted, "expected Refuted, got {:?} witness={}", c.verdict, c.witness);
        assert!(c.witness.contains("error"), "witness must contain kernel diagnostic: {}", c.witness);
        assert!(c.exit_code.unwrap_or(0) != 0);
    }
}