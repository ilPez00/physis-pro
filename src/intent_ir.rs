use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const IR_VERSION: u32 = 1;
pub const SLOTS: [&str; 9] = [
    "goal",
    "objects",
    "constraints",
    "context",
    "required_state",
    "operations",
    "preferred_tools",
    "safety",
    "success_condition",
];

pub const READ_ONLY_OPS: [&str; 6] = [
    "show",
    "search",
    "read",
    "inspect",
    "explain",
    "resume_view",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentIR {
    pub version: u32,
    pub user: String,
    pub source_utterance: String,
    pub compiled_by: String,
    pub slots: HashMap<String, Option<serde_json::Value>>,
    pub nil_slots: Vec<String>,
}

impl IntentIR {
    pub fn new(
        raw: HashMap<String, serde_json::Value>,
        source_utterance: String,
        user: String,
        compiled_by: Option<String>,
    ) -> Self {
        let mut slots = HashMap::new();
        let mut nil_slots = Vec::new();

        for s in SLOTS {
            match raw.get(s) {
                // A slot is nil when it is absent, JSON null, or the empty
                // string. A non-string value (a number, say) is present.
                Some(v) if !v.is_null() && v.as_str() != Some("") => {
                    slots.insert(s.to_string(), Some(v.clone()));
                }
                _ => {
                    slots.insert(s.to_string(), None);
                    nil_slots.push(s.to_string());
                }
            }
        }

        Self {
            version: IR_VERSION,
            user,
            source_utterance,
            compiled_by: compiled_by.unwrap_or_else(|| "fixed-gate-v1".to_string()),
            slots,
            nil_slots,
        }
    }

    pub fn has_nil(&self) -> bool {
        !self.nil_slots.is_empty()
    }

    pub fn bind_tasks(&self) -> Vec<BindTask> {
        self.nil_slots
            .iter()
            .map(|s| BindTask {
                slot: s.clone(),
                action: "observe_and_bind".to_string(),
                user: self.user.clone(),
                utterance: self.source_utterance.clone(),
            })
            .collect()
    }

    pub fn is_read_only(&self) -> bool {
        let ops = self
            .slots
            .get("operations")
            .and_then(|v| v.as_ref())
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        ops.iter().all(|o| READ_ONLY_OPS.contains(o))
    }

    pub fn render(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!(
            "IntentIR v{} compiled_by={} user={}",
            self.version, self.compiled_by, self.user
        ));
        for s in SLOTS {
            let value = self.slots.get(s).and_then(|v| v.as_ref());
            let mark = if self.nil_slots.contains(&s.to_string()) {
                " <NIL: bind task>"
            } else {
                ""
            };
            let val_str = match value {
                Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "null".to_string()),
                None => "null".to_string(),
            };
            lines.push(format!("  {}: {}{}", s, val_str, mark));
        }
        lines.join("\n")
    }

    pub fn to_dict(&self) -> serde_json::Value {
        let mut slots_serialized = serde_json::Map::new();
        for (k, v) in &self.slots {
            slots_serialized.insert(k.clone(), v.clone().unwrap_or(serde_json::Value::Null));
        }

        serde_json::json!({
            "version": self.version,
            "user": self.user,
            "source_utterance": self.source_utterance,
            "compiled_by": self.compiled_by,
            "slots": slots_serialized,
            "nil_slots": self.nil_slots,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindTask {
    pub slot: String,
    pub action: String,
    pub user: String,
    pub utterance: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_version_recorded() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("continue positional attention experiment")),
                ("operations".to_string(), serde_json::json!(["show"])),
                ("objects".to_string(), serde_json::json!(["repo:physis"])),
            ]),
            "continue the positional attention experiment".to_string(),
            "gio".to_string(),
            None,
        );
        assert_eq!(ir.version, 1);
    }

    #[test]
    fn test_ir_nil_slots_detected() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("continue positional attention experiment")),
                ("operations".to_string(), serde_json::json!(["show"])),
                ("objects".to_string(), serde_json::json!(["repo:physis"])),
            ]),
            "continue the positional attention experiment".to_string(),
            "gio".to_string(),
            None,
        );
        assert!(ir.has_nil());
        assert!(ir.nil_slots.contains(&"constraints".to_string()));
    }

    #[test]
    fn test_ir_read_only_for_show() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("continue positional attention experiment")),
                ("operations".to_string(), serde_json::json!(["show"])),
                ("objects".to_string(), serde_json::json!(["repo:physis"])),
            ]),
            "continue the positional attention experiment".to_string(),
            "gio".to_string(),
            None,
        );
        assert!(ir.is_read_only());
    }

    #[test]
    fn test_ir_bind_tasks_count() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("continue positional attention experiment")),
                ("operations".to_string(), serde_json::json!(["show"])),
                ("objects".to_string(), serde_json::json!(["repo:physis"])),
            ]),
            "continue the positional attention experiment".to_string(),
            "gio".to_string(),
            None,
        );
        assert_eq!(ir.bind_tasks().len(), ir.nil_slots.len());
    }

    #[test]
    fn test_ir_provenance_compiled_by() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("continue positional attention experiment")),
                ("operations".to_string(), serde_json::json!(["show"])),
                ("objects".to_string(), serde_json::json!(["repo:physis"])),
            ]),
            "continue the positional attention experiment".to_string(),
            "gio".to_string(),
            None,
        );
        assert_eq!(ir.compiled_by, "fixed-gate-v1");
    }

    #[test]
    fn test_ir_execute_not_read_only() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("build")),
                ("operations".to_string(), serde_json::json!(["execute"])),
                ("success_condition".to_string(), serde_json::json!("exit zero")),
            ]),
            "build physis".to_string(),
            "gio".to_string(),
            None,
        );
        assert!(!ir.is_read_only());
    }

    #[test]
    fn test_ir_render_shows_nil_marker() {
        let ir = IntentIR::new(
            HashMap::from([
                ("goal".to_string(), serde_json::json!("build")),
                ("operations".to_string(), serde_json::json!(["execute"])),
                ("success_condition".to_string(), serde_json::json!("exit zero")),
            ]),
            "build physis".to_string(),
            "gio".to_string(),
            None,
        );
        let rendered = ir.render();
        assert!(rendered.contains("constraints"));
        assert!(rendered.contains("NIL"));
    }
}