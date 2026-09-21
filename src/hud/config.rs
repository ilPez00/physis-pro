//! Layered HUD configuration: builtin < user < workspace < flags.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One configuration layer, in increasing precedence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigLayer {
    Builtin,
    User,
    Workspace,
    Flags,
}

/// A dotted key with the layer that set it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredKey {
    pub key: String,
    pub layer: ConfigLayer,
}

/// Four JSON documents, lowest precedence first.
#[derive(Debug, Clone, Default)]
pub struct HudConfig {
    builtin: serde_json::Value,
    user: serde_json::Value,
    workspace: serde_json::Value,
    flags: serde_json::Value,
}

impl HudConfig {
    pub fn new(
        builtin: serde_json::Value,
        user: serde_json::Value,
        workspace: serde_json::Value,
        flags: serde_json::Value,
    ) -> Self {
        Self { builtin, user, workspace, flags }
    }

    /// Load the file-backed layers for a workspace root. Missing files are
    /// empty layers; a malformed file is reported and ignored rather than
    /// taking the HUD down (same contract as `Settings::load`).
    pub fn load(root: &Path) -> (Self, Vec<String>) {
        let mut warnings = Vec::new();
        let builtin = crate::settings::Settings::defaults();
        let builtin_val = serde_json::to_value(&builtin).expect("shipped settings serialize");
        let user = read_layer(&Self::user_path(), &mut warnings);
        let workspace = read_layer(&root.join(".physis").join("hud.json"), &mut warnings);
        (Self::new(builtin_val, user, workspace, serde_json::Value::Null), warnings)
    }

    pub fn user_path() -> PathBuf {
        if let Ok(p) = std::env::var("PHYSIS_HUD") {
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".physis").join("hud.json")
    }

    /// Merge one document over another, object-wise recursively. Arrays and
    /// scalars replace; objects merge per key. Null means "no opinion": it
    /// never blanks an object beneath it. Pure.
    pub fn merge(base: &serde_json::Value, over: &serde_json::Value) -> serde_json::Value {
        if over.is_null() {
            return base.clone();
        }
        match (base, over) {
            (serde_json::Value::Object(b), serde_json::Value::Object(o)) => {
                let mut out = b.clone();
                for (k, v) in o {
                    let merged = match out.get(k) {
                        Some(existing) => Self::merge(existing, v),
                        None => v.clone(),
                    };
                    out.insert(k.clone(), merged);
                }
                serde_json::Value::Object(out)
            }
            (_, v) => v.clone(),
        }
    }

    /// The effective document: builtin < user < workspace < flags. A Null
    /// layer is "no opinion", not "delete everything": merging Null over an
    /// object keeps the object, so a missing user file never blanks the
    /// shipped defaults.
    pub fn effective(&self) -> serde_json::Value {
        let m = Self::merge(&self.builtin, &self.user);
        let m = Self::merge(&m, &self.workspace);
        Self::merge(&m, &self.flags)
    }

    /// Which layer set a dotted key, walking top-down. Builtin when no layer
    /// overrides the default.
    pub fn layer_of(&self, dotted: &str) -> ConfigLayer {
        let parts: Vec<&str> = dotted.split('.').collect();
        if get_path(&self.flags, &parts).is_some() {
            return ConfigLayer::Flags;
        }
        if get_path(&self.workspace, &parts).is_some() {
            return ConfigLayer::Workspace;
        }
        if get_path(&self.user, &parts).is_some() {
            return ConfigLayer::User;
        }
        ConfigLayer::Builtin
    }

    /// Dirty keys with the layer each belongs to, so the save path writes
    /// each key back to its own layer's file.
    pub fn dirty_layers(&self, dirty: &BTreeMap<String, String>) -> Vec<LayeredKey> {
        let mut out: Vec<LayeredKey> = dirty
            .keys()
            .map(|key| LayeredKey { key: key.clone(), layer: self.layer_of(key) })
            .collect();
        out.sort_by(|a, b| a.key.cmp(&b.key));
        out
    }
}

fn read_layer(path: &Path, warnings: &mut Vec<String>) -> serde_json::Value {
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                warnings.push(format!("{}: {e}; layer ignored", path.display()));
                serde_json::Value::Null
            }
        },
        Err(_) => serde_json::Value::Null,
    }
}

fn get_path<'a>(doc: &'a serde_json::Value, parts: &[&str]) -> Option<&'a serde_json::Value> {
    let mut cur = doc;
    for part in parts {
        cur = cur.get(*part)?;
    }
    Some(cur)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn later_layers_win_and_objects_merge_per_key() {
        let base = json!({"a": 1, "nested": {"x": 1, "y": 2}});
        let over = json!({"nested": {"y": 9}, "b": true});
        let merged = HudConfig::merge(&base, &over);
        assert_eq!(merged["a"], json!(1));
        assert_eq!(merged["b"], json!(true));
        assert_eq!(merged["nested"], json!({"x": 1, "y": 9}));
    }

    #[test]
    fn effective_follows_layer_order() {
        let cfg = HudConfig::new(
            json!({"k": "builtin", "stay": 1}),
            json!({"k": "user"}),
            json!({"k": "workspace"}),
            json!({"k": "flags"}),
        );
        let eff = cfg.effective();
        assert_eq!(eff["k"], json!("flags"));
        assert_eq!(eff["stay"], json!(1));
        assert_eq!(cfg.layer_of("k"), ConfigLayer::Flags);
        assert_eq!(cfg.layer_of("stay"), ConfigLayer::Builtin);
    }

    #[test]
    fn saves_write_back_to_the_key_own_layer() {
        let cfg = HudConfig::new(json!({"a": 1}), json!({"u": 1}), json!({"w": 1}), serde_json::Value::Null);
        let mut dirty = BTreeMap::new();
        dirty.insert("u".to_string(), "2".to_string());
        dirty.insert("w".to_string(), "3".to_string());
        dirty.insert("fresh".to_string(), "4".to_string());
        let layers = cfg.dirty_layers(&dirty);
        let by_key: BTreeMap<&str, ConfigLayer> =
            layers.iter().map(|l| (l.key.as_str(), l.layer)).collect();
        assert_eq!(by_key["u"], ConfigLayer::User);
        assert_eq!(by_key["w"], ConfigLayer::Workspace);
        assert_eq!(by_key["fresh"], ConfigLayer::Builtin);
    }

    #[test]
    fn malformed_layer_is_ignored_not_fatal() {
        let dir = std::env::temp_dir().join("physis-hud-config-test");
        std::fs::create_dir_all(dir.join(".physis")).ok();
        std::fs::write(dir.join(".physis").join("hud.json"), "{not json").ok();
        let (cfg, warnings) = HudConfig::load(&dir);
        assert!(!warnings.is_empty(), "the gap is named, not silent");
        assert!(cfg.effective().is_object());
        std::fs::remove_dir_all(&dir).ok();
    }
}
