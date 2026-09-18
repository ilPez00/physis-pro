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
        self.objects.insert(
            oid,
            RegistryObject { kind, path, meta },
        );
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

    pub fn call(&mut self, oid: &str, verb: &str, kwargs: HashMap<String, serde_json::Value>) -> CallResult {
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
        let limit = kwargs
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(4000) as usize;

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

        (true, Some(serde_json::Value::String(content)), "read_ok".to_string())
    }

    fn fs_write_runner(
        &self,
        obj: &RegistryObject,
        kwargs: &HashMap<String, serde_json::Value>,
    ) -> (bool, Option<serde_json::Value>, String) {
        let content = kwargs
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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
            std::fs::OpenOptions::new().create(true).append(true).open(&draft_path)
        } else {
            std::fs::OpenOptions::new().create(true).write(true).truncate(true).open(&draft_path)
        };

        match mode {
            Ok(mut file) => {
                use std::io::Write;
                if let Err(e) = file.write_all(content.as_bytes()) {
                    return (false, None, format!("write error: {}", e));
                }
                (true, Some(serde_json::Value::String(draft_path)), "draft_created".to_string())
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
    detail.insert("exit".to_string(), serde_json::Value::Number(exit_code.into()));
    (exit_code == 0, detail)
}

pub fn v_path_exists(
    raw: &Option<serde_json::Value>,
    _obj: &RegistryObject,
    _kwargs: &HashMap<String, serde_json::Value>,
) -> (bool, HashMap<String, serde_json::Value>) {
    let path = raw
        .as_ref()
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let exists = std::path::Path::new(path).exists();
    let mut detail = HashMap::new();
    detail.insert("path".to_string(), serde_json::Value::String(path.to_string()));
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
            meta.insert("path".to_string(), serde_json::Value::String(path.to_string_lossy().to_string()));
            reg.register_object(name, kind.to_string(), path.to_string_lossy().to_string(), meta);
        }
    }

    reg.add_capability(Capability::new(
        "fs.read".to_string(),
        vec!["dir".to_string(), "repo".to_string(), "file".to_string()],
        vec!["read".to_string()],
        "api".to_string(),
        "reversible".to_string(),
        HashMap::from([("latency_p50_s".to_string(), 0.1)]),
        None,
    ));

    reg.add_capability(Capability::new(
        "fs.write_draft".to_string(),
        vec!["file".to_string(), "dir".to_string(), "repo".to_string()],
        vec!["write".to_string()],
        "api".to_string(),
        "draft_only".to_string(),
        HashMap::from([("latency_p50_s".to_string(), 0.1)]),
        None,
    ));

    reg.add_capability(Capability::new(
        "shell.execute".to_string(),
        vec!["dir".to_string(), "repo".to_string()],
        vec!["execute".to_string()],
        "cli".to_string(),
        "irreversible_without_approval".to_string(),
        HashMap::from([("latency_p50_s".to_string(), 1.0)]),
        None,
    ));

    reg.add_capability(Capability::new(
        "git.status".to_string(),
        vec!["repo".to_string()],
        vec!["inspect".to_string()],
        "cli".to_string(),
        "reversible".to_string(),
        HashMap::from([("latency_p50_s".to_string(), 0.5)]),
        None,
    ));

    reg.add_capability(Capability::new(
        "git.diff".to_string(),
        vec!["repo".to_string()],
        vec!["read".to_string()],
        "cli".to_string(),
        "reversible".to_string(),
        HashMap::from([("latency_p50_s".to_string(), 0.5)]),
        None,
    ));
}