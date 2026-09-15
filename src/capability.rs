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
    "read",
    "write",
    "execute",
    "inspect",
    "search",
    "show",
    "undo",
    "create",
    "delete",
    "modify",
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