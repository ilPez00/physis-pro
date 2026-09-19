use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub name: String,
    pub object_kinds: Vec<String>,
    pub verbs: Vec<String>,
    pub tier: String,
    pub risk: String,
    pub cost: HashMap<String, f64>,
    pub reliability_alpha: u64,
    pub reliability_beta: u64,
    pub undo_link: Option<String>,
}

impl Capability {
    pub fn new(
        name: String,
        object_kinds: Vec<String>,
        verbs: Vec<String>,
        tier: String,
        risk: String,
        cost: HashMap<String, f64>,
        undo_link: Option<String>,
    ) -> Self {
        Self {
            name,
            object_kinds,
            verbs,
            tier,
            risk,
            cost,
            reliability_alpha: 1,
            reliability_beta: 1,
            undo_link,
        }
    }

    pub fn reliability_mean(&self) -> f64 {
        self.reliability_alpha as f64 / (self.reliability_alpha + self.reliability_beta) as f64
    }

    pub fn record(&mut self, ok: bool) {
        if ok {
            self.reliability_alpha += 1;
        } else {
            self.reliability_beta += 1;
        }
    }

    pub fn has_verb(&self, verb: &str) -> bool {
        self.verbs.contains(&verb.to_string())
    }

    pub fn supports_kind(&self, kind: &str) -> bool {
        self.object_kinds.contains(&kind.to_string())
    }
}

pub const VERB_TIMEOUT: u64 = 15;

pub const TEN_VERBS: [&str; 10] = [
    "read", "write", "execute", "inspect", "search", "show", "undo", "create", "delete", "modify",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservationRecord {
    pub t: f64,
    pub actor: String,
    pub object: String,
    pub verb: String,
    pub capability: Option<String>,
    pub ok: bool,
    pub detail: String,
    pub reliability: Option<f64>,
}

pub struct Observations {
    path: std::path::PathBuf,
}

impl Observations {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn append(&self, record: ObservationRecord) {
        let mut record = record;
        record.t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64();
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            use std::io::Write;
            let _ = writeln!(file, "{}", serde_json::to_string(&record).unwrap());
        }
    }

    pub fn tail(&self, n: usize) -> Vec<ObservationRecord> {
        let content = std::fs::read_to_string(&self.path).unwrap_or_default();
        let mut records: Vec<ObservationRecord> = content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect();
        if records.len() > n {
            records.split_off(records.len() - n)
        } else {
            records
        }
    }
}

// ---------------------------------------------------------------------------
// Config-driven capabilities (config/actions.json)
// ---------------------------------------------------------------------------

/// One capability as it appears in `config/actions.json`. This is the data
/// shape, deliberately separate from [`Capability`] so the JSON does not have
/// to carry runtime reliability counters; the loader builds a fresh
/// `Capability` per spec with the uniform prior `alpha = beta = 1`.
#[derive(Debug, Clone, Deserialize)]
struct ActionSpec {
    name: String,
    object_kinds: Vec<String>,
    verbs: Vec<String>,
    tier: String,
    risk: String,
    #[serde(default)]
    cost: HashMap<String, f64>,
    #[serde(default)]
    undo_link: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ActionFile {
    #[serde(default = "default_actions_version")]
    version: u32,
    capabilities: Vec<ActionSpec>,
}

fn default_actions_version() -> u32 {
    1
}

/// Build `Capability` values from the `config/actions.json` text. Pure: the
/// caller owns the bytes, so tests and callers can pass either the embedded
/// default or a file read at runtime.
pub fn load_capabilities(json: &str) -> Result<Vec<Capability>, String> {
    let file: ActionFile = serde_json::from_str(json).map_err(|e| format!("actions.json: {e}"))?;
    if file.version != 1 {
        return Err(format!(
            "actions.json: unsupported version {} (this build reads 1)",
            file.version
        ));
    }
    Ok(file
        .capabilities
        .into_iter()
        .map(|spec| {
            Capability::new(
                spec.name,
                spec.object_kinds,
                spec.verbs,
                spec.tier,
                spec.risk,
                spec.cost,
                spec.undo_link,
            )
        })
        .collect())
}

/// Read and parse an actions file from disk. A missing file is an error the
/// caller reports, never a silent empty registry.
pub fn load_capabilities_file(path: &std::path::Path) -> Result<Vec<Capability>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    load_capabilities(&text)
}

/// The capabilities this binary ships with. Compiled in from
/// `config/actions.json` so a missing working directory cannot silently empty
/// the registry; a malformed file is a build bug and fails loudly.
pub fn default_capabilities() -> Vec<Capability> {
    const ACTIONS_JSON: &str = include_str!("../config/actions.json");
    load_capabilities(ACTIONS_JSON).expect("config/actions.json is compiled in and must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_actions_file_parses_and_names_the_known_capabilities() {
        let caps = default_capabilities();
        let names: Vec<&str> = caps.iter().map(|c| c.name.as_str()).collect();
        for expected in [
            "fs.read",
            "fs.write_draft",
            "shell.execute",
            "git.status",
            "git.diff",
        ] {
            assert!(
                names.contains(&expected),
                "missing capability {expected} in {names:?}"
            );
        }
        // Fresh capabilities carry the uniform prior, not a zeroed one.
        for cap in &caps {
            assert_eq!(cap.reliability_alpha, 1);
            assert_eq!(cap.reliability_beta, 1);
            assert!(!cap.verbs.is_empty());
            assert!(!cap.object_kinds.is_empty());
        }
    }

    #[test]
    fn malformed_json_is_an_error_not_an_empty_registry() {
        assert!(load_capabilities("{ not json").is_err());
        assert!(load_capabilities("{\"capabilities\":[{\"name\":\"x\"}]}").is_err());
    }

    #[test]
    fn reliability_moves_only_through_record() {
        let fresh = || {
            Capability::new(
                "t".into(),
                vec!["file".into()],
                vec!["read".into()],
                "api".into(),
                "reversible".into(),
                HashMap::new(),
                None,
            )
        };

        let mut ok = fresh();
        assert_eq!(ok.reliability_mean(), 0.5);
        ok.record(true);
        assert!(ok.reliability_mean() > 0.5);

        let mut bad = fresh();
        bad.record(false);
        assert!(bad.reliability_mean() < 0.5);
    }
}
