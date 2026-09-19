use crate::capability::{Capability, ObservationRecord, Observations};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryObject {
    pub kind: String,
    pub path: String,
    #[serde(flatten)]
    pub meta: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallResult {
    pub ok: bool,
    pub result: Option<serde_json::Value>,
    pub validation: String,
    pub capability: String,
    pub error: Option<String>,
}

pub struct Registry {
    obs: Observations,
    objects: HashMap<String, RegistryObject>,
    caps: Vec<Capability>,
}

impl Registry {
    pub fn new(obs_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            obs: Observations::new(obs_path),
            objects: HashMap::new(),
            caps: Vec::new(),
        }
    }

    pub fn register_object(
        &mut self,
        oid: String,
        kind: String,
        path: String,
        meta: HashMap<String, serde_json::Value>,
    ) {
        self.objects
            .insert(oid, RegistryObject { kind, path, meta });
    }

    pub fn add_capability(&mut self, cap: Capability) {
        self.caps.push(cap);
    }

    pub fn capabilities_for(&self, oid: &str, verb: &str) -> Vec<&Capability> {
        let obj = match self.objects.get(oid) {
            Some(o) => o,
            None => return Vec::new(),
        };
        self.caps
            .iter()
            .filter(|c| c.object_kinds.contains(&obj.kind) && c.has_verb(verb))
            .collect()
    }

    pub fn call(
        &mut self,
        oid: &str,
        verb: &str,
        kwargs: HashMap<String, serde_json::Value>,
    ) -> CallResult {
        let caps = self.capabilities_for(oid, verb);
        if caps.is_empty() {
            self.obs.append(ObservationRecord {
                t: 0.0,
                actor: "registry".to_string(),
                object: oid.to_string(),
                verb: verb.to_string(),
                capability: None,
                ok: false,
                detail: "unavailable: no capability".to_string(),
                reliability: None,
            });
            return CallResult {
                ok: false,
                result: None,
                validation: "unavailable: no capability".to_string(),
                capability: "none".to_string(),
                error: Some("unavailable".to_string()),
            };
        }

        let cap = caps[0];
        let obj = self.objects.get(oid).unwrap().clone();
        let cap_name = cap.name.clone();

        // Deny by default: a capability whose risk is irreversible does not run
        // until the caller states confirmation explicitly. The refusal is
        // recorded like any other outcome, so it is never silent and never a
        // success. (`confirm` is the magic-textbox form of "I mean this".)
        if cap.risk.starts_with("irreversible")
            && !kwargs
                .get("confirm")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            let detail = format!(
                "refused: `{}` is {} and requires confirm=true",
                cap.name, cap.risk
            );
            self.obs.append(ObservationRecord {
                t: 0.0,
                actor: "registry".to_string(),
                object: oid.to_string(),
                verb: verb.to_string(),
                capability: Some(cap_name.clone()),
                ok: false,
                detail: detail.clone(),
                reliability: None,
            });
            return CallResult {
                ok: false,
                result: None,
                validation: detail,
                capability: cap_name,
                error: Some("approval_required".to_string()),
            };
        }

        let (ok, raw, detail) = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.run_capability(cap, &obj, &kwargs)
        })) {
            Ok(result) => result,
            Err(e) => {
                let detail = format!("adapter_error: {:?}", e);
                (false, None, detail)
            }
        };

        let cap_mut = self.caps.iter_mut().find(|c| c.name == cap_name).unwrap();
        cap_mut.record(ok);
        let reliability = cap_mut.reliability_mean();

        self.obs.append(ObservationRecord {
            t: 0.0,
            actor: "registry".to_string(),
            object: oid.to_string(),
            verb: verb.to_string(),
            capability: Some(cap_name.clone()),
            ok,
            detail: detail.clone(),
            reliability: Some(reliability),
        });

        let raw_clone = raw.clone();
        let mut result = CallResult {
            ok,
            result: raw,
            validation: detail.clone(),
            capability: cap_name,
            error: None,
        };

        if !ok {
            result.error = Some(if raw_clone.is_some() {
                "validation_failed".to_string()
            } else {
                "adapter_error".to_string()
            });
        }

        result
    }

    fn run_capability(
        &self,
        cap: &Capability,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let (raw, _detail) = match cap.name.as_str() {
            "fs.read" => {
                let (_, raw, detail) = self.fs_read_runner(obj, kwargs);
                (raw, detail)
            }
            "fs.write_draft" => {
                let (_, raw, detail) = self.fs_write_runner(obj, kwargs);
                (raw, detail)
            }
            "shell.execute" => {
                let (_, raw, detail) = self.shell_runner(obj, kwargs);
                (raw, detail)
            }
            "git.status" | "git.diff" => {
                let (_, raw, detail) = self.git_runner(obj, kwargs);
                (raw, detail)
            }
            _ => return (false, None, format!("unknown capability: {}", cap.name)),
        };

        let (ok, validation_detail) = self.validate_capability(&cap.name, &raw, obj, kwargs);
        (ok, raw, validation_detail)
    }

    fn validate_capability(
        &self,
        cap_name: &str,
        raw: &Option<serde_json::Value>,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, String) {
        match cap_name {
            "shell.execute" => {
                let (ok, detail) = v_exit_zero(raw, obj, kwargs);
                (ok, format!("{:?}", detail))
            }
            "fs.write_draft" => {
                let (ok, detail) = v_path_exists(raw, obj, kwargs);
                (ok, format!("{:?}", detail))
            }
            "fs.read" => (true, "read_ok".to_string()),
            "git.status" | "git.diff" => {
                let (ok, detail) = v_exit_zero(raw, obj, kwargs);
                (ok, format!("{:?}", detail))
            }
            _ => (false, "no validator".to_string()),
        }
    }

    fn fs_read_runner(
        &self,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let path = std::path::Path::new(&obj.path);
        let limit = kwargs.get("limit").and_then(|v| v.as_u64()).unwrap_or(4000) as usize;

        let content = if path.is_dir() {
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    let mut names: Vec<String> = entries
                        .filter_map(|e| e.ok())
                        .map(|e| e.file_name().to_string_lossy().to_string())
                        .collect();
                    names.sort();
                    names.into_iter().take(100).collect::<Vec<_>>().join("\n")
                }
                Err(e) => return (false, None, format!("read_dir error: {}", e)),
            }
        } else {
            match std::fs::read_to_string(path) {
                Ok(s) => {
                    if s.len() > limit {
                        s[..limit].to_string()
                    } else {
                        s
                    }
                }
                Err(e) => return (false, None, format!("read error: {}", e)),
            }
        };

        (
            true,
            Some(serde_json::Value::String(content)),
            "read_ok".to_string(),
        )
    }

    fn fs_write_runner(
        &self,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let content = kwargs.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let append = kwargs
            .get("append")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let draft_path = if append {
            format!("{}.draft.log", obj.path)
        } else {
            format!("{}.draft", obj.path)
        };

        let mode = if append {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&draft_path)
        } else {
            std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&draft_path)
        };

        match mode {
            Ok(mut file) => {
                use std::io::Write;
                if let Err(e) = file.write_all(content.as_bytes()) {
                    return (false, None, format!("write error: {}", e));
                }
                (
                    true,
                    Some(serde_json::Value::String(draft_path)),
                    "draft_created".to_string(),
                )
            }
            Err(e) => (false, None, format!("open error: {}", e)),
        }
    }

    fn shell_runner(
        &self,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let cmd = match kwargs.get("cmd").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return (false, None, "missing cmd".to_string()),
        };

        let cwd = obj.meta.get("path").and_then(|v| v.as_str()).unwrap_or(".");

        let output = match Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(cwd)
            .output()
        {
            Ok(o) => o,
            Err(e) => return (false, None, format!("spawn error: {}", e)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        });

        (true, Some(result), "executed".to_string())
    }

    fn git_runner(
        &self,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let args = kwargs
            .get("args")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| vec!["status".to_string(), "--short".to_string()]);

        let output = match Command::new("git")
            .args(&args)
            .current_dir(&obj.path)
            .output()
        {
            Ok(o) => o,
            Err(e) => return (false, None, format!("git spawn error: {}", e)),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = serde_json::json!({
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        });

        (true, Some(result), "executed".to_string())
    }

    pub fn obs_tail(&self, n: usize) -> Vec<ObservationRecord> {
        self.obs.tail(n)
    }

    pub fn get_capability(&self, name: &str) -> Option<&Capability> {
        self.caps.iter().find(|c| c.name == name)
    }
}

pub fn v_exit_zero(
    raw: &Option<serde_json::Value>,
    _obj: &RegistryObject,
    _kwargs: &HashMap<String, serde_json::Value>,
) -> (bool, HashMap<String, serde_json::Value>) {
    let exit_code = raw
        .as_ref()
        .and_then(|r| r.get("exit_code"))
        .and_then(|v| v.as_i64())
        .unwrap_or(-1);
    let mut detail = HashMap::new();
    detail.insert(
        "exit".to_string(),
        serde_json::Value::Number(exit_code.into()),
    );
    (exit_code == 0, detail)
}

pub fn v_path_exists(
    raw: &Option<serde_json::Value>,
    _obj: &RegistryObject,
    _kwargs: &HashMap<String, serde_json::Value>,
) -> (bool, HashMap<String, serde_json::Value>) {
    let path = raw.as_ref().and_then(|r| r.as_str()).unwrap_or("");
    let exists = std::path::Path::new(path).exists();
    let mut detail = HashMap::new();
    detail.insert(
        "path".to_string(),
        serde_json::Value::String(path.to_string()),
    );
    (exists, detail)
}

pub fn seed(reg: &mut Registry, root: &str) {
    let entries = match std::fs::read_dir(root) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let path = entry.path();
        if path.is_dir() && !name.starts_with('.') {
            let kind = if path.join(".git").exists() {
                "repo"
            } else {
                "dir"
            };
            let mut meta = HashMap::new();
            meta.insert(
                "path".to_string(),
                serde_json::Value::String(path.to_string_lossy().to_string()),
            );
            reg.register_object(
                name,
                kind.to_string(),
                path.to_string_lossy().to_string(),
                meta,
            );
        }
    }

    for cap in crate::capability::default_capabilities() {
        reg.add_capability(cap);
    }
}

/// Default append-only observation path: the same data dir the nav workspace
/// uses, so the registry's record sits beside the index rather than inventing a
/// second store.
pub fn default_obs_path() -> std::path::PathBuf {
    let dir = std::env::var("PHYSIS_DATA_DIR")
        .ok()
        .filter(|p| !p.is_empty())
        .unwrap_or_else(|| ".physis".to_string());
    std::path::PathBuf::from(dir).join("registry_observations.jsonl")
}

impl Registry {
    /// Register every object in a nav index under the `fs:` namespace. Kind is
    /// the object kind in lower case, which is the vocabulary `config/actions.json`
    /// matches against; `path` is the source path so the shell executor has a
    /// working directory.
    pub fn register_nav_index(&mut self, index: &crate::nav::NavIndex) {
        for obj in index.objects.values() {
            let kind = match obj.kind {
                crate::nav::ObjectKind::File => "file",
                crate::nav::ObjectKind::Passage => "passage",
                crate::nav::ObjectKind::Directory => "dir",
                crate::nav::ObjectKind::Concept => "concept",
                crate::nav::ObjectKind::Note => "note",
                crate::nav::ObjectKind::Other => "other",
            };
            let path = obj
                .source
                .as_ref()
                .map(|s| s.path.clone())
                .unwrap_or_default();
            let mut meta = HashMap::new();
            meta.insert("path".to_string(), serde_json::Value::String(path.clone()));
            meta.insert(
                "label".to_string(),
                serde_json::Value::String(obj.label.clone()),
            );
            self.register_object(obj.id.short(), kind.to_string(), path, meta);
        }
    }

    /// One host object and one device object placeholder. They carry no
    /// capability yet, so every verb on them resolves `unavailable` — honest
    /// until S12 gives hosts and devices typed actions.
    pub fn register_placeholders(&mut self) {
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| "localhost".to_string());
        let mut host_meta = HashMap::new();
        host_meta.insert("hostname".to_string(), serde_json::Value::String(hostname));
        self.register_object(
            "host:local".to_string(),
            "host".to_string(),
            ".".to_string(),
            host_meta,
        );

        let mut device_meta = HashMap::new();
        device_meta.insert(
            "note".to_string(),
            serde_json::Value::String("placeholder; no device adapter wired".to_string()),
        );
        self.register_object(
            "device:placeholder".to_string(),
            "device".to_string(),
            String::new(),
            device_meta,
        );
    }

    /// Build a registry for a workspace root: open the nav workspace, register
    /// its `fs:` objects plus the host/device placeholders, and load the
    /// capabilities from the shipped `config/actions.json`.
    pub fn from_workspace(root: &std::path::Path) -> anyhow::Result<Self> {
        let ws = crate::nav::workspace::NavWorkspace::open(root)?;
        let mut reg = Registry::new(default_obs_path());
        reg.register_workspace(&ws);
        Ok(reg)
    }

    /// Populate a registry from an already-open workspace: `fs:` objects, the
    /// host/device placeholders, and the shipped capabilities. The HUD already
    /// holds the workspace, so it calls this rather than reopening sled.
    pub fn register_workspace(&mut self, ws: &crate::nav::workspace::NavWorkspace) {
        self.register_nav_index(ws.index());
        self.register_placeholders();
        for cap in crate::capability::default_capabilities() {
            self.add_capability(cap);
        }
    }

    /// Capability names and their verbs, for `action list`.
    pub fn list_capabilities(&self) -> Vec<(String, Vec<String>, String, String)> {
        self.caps
            .iter()
            .map(|c| {
                (
                    c.name.clone(),
                    c.verbs.clone(),
                    c.risk.clone(),
                    c.tier.clone(),
                )
            })
            .collect()
    }

    /// Registered objects as `(oid, kind, path)`, for `action list`.
    pub fn list_objects(&self) -> Vec<(String, String, String)> {
        let mut out: Vec<(String, String, String)> = self
            .objects
            .iter()
            .map(|(oid, o)| (oid.clone(), o.kind.clone(), o.path.clone()))
            .collect();
        out.sort();
        out
    }
}
