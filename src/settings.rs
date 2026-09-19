//! User-editable settings: command syntax and provider configuration.
//!
//! Two layers, user wins: the shipped defaults compiled in from
//! `config/settings.json`, and the user's file at `PHYSIS_SETTINGS` or
//! `~/.physis/settings.json`. Editing the user file changes how commands are
//! spelled and which model a provider uses, without a rebuild.
//!
//! Keys are stored as the **name of an environment variable**, never the secret
//! itself, so a settings file can be read, shared or committed without leaking
//! anything. `settings set providers.jev.key_env MY_JEV_KEY` names the variable;
//! the value still comes from the environment.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One command: its canonical word, the words that alias to it, and — for a
/// command that addresses an agent — which agent it targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandSpec {
    /// Alternative words that route to the same command.
    #[serde(default)]
    pub aliases: Vec<String>,
    /// The agent this command addresses, when it is an agent command.
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CommandSpec {
    /// A new command is enabled by default: `settings set commands.x.aliases …`
    /// should not create a command that is silently off.
    fn default() -> Self {
        Self {
            aliases: Vec::new(),
            agent: None,
            enabled: true,
        }
    }
}

/// One provider: how to reach an agent. Every field is optional so a partial
/// override in the user file changes only what it names.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProviderSpec {
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub bin: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    /// Name of the environment variable holding the key. Never the key.
    #[serde(default)]
    pub key_env: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Canonical command word → spec.
    #[serde(default)]
    pub commands: BTreeMap<String, CommandSpec>,
    /// Agent id → provider spec.
    #[serde(default)]
    pub providers: BTreeMap<String, ProviderSpec>,
}

fn default_version() -> u32 {
    1
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            commands: BTreeMap::new(),
            providers: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// Parse a settings document, rejecting an unknown version.
    pub fn parse(json: &str) -> Result<Self, String> {
        let s: Settings = serde_json::from_str(json).map_err(|e| format!("settings.json: {e}"))?;
        if s.version != 1 {
            return Err(format!(
                "settings.json: unsupported version {} (this build reads 1)",
                s.version
            ));
        }
        Ok(s)
    }

    /// The shipped defaults, compiled in so a missing file cannot silently
    /// empty the command table.
    pub fn defaults() -> Self {
        const SETTINGS_JSON: &str = include_str!("../config/settings.json");
        Self::parse(SETTINGS_JSON).expect("config/settings.json is compiled in and must parse")
    }

    /// Where the user's overrides live.
    pub fn user_path() -> PathBuf {
        if let Ok(p) = std::env::var("PHYSIS_SETTINGS") {
            if !p.is_empty() {
                return PathBuf::from(p);
            }
        }
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home).join(".physis").join("settings.json")
    }

    /// Load defaults, then layer the user file over them. A missing user file
    /// is the normal case; a malformed one is reported and ignored rather than
    /// taking the command table down with it.
    pub fn load() -> Self {
        let mut s = Self::defaults();
        let path = Self::user_path();
        if let Ok(text) = std::fs::read_to_string(&path) {
            match Self::parse(&text) {
                Ok(user) => s.merge(user),
                Err(e) => eprintln!("physis: {e}; using shipped defaults"),
            }
        }
        s
    }

    /// Layer `other` over `self`: commands and providers merge per key, and a
    /// provider's named fields override only those fields.
    pub fn merge(&mut self, other: Settings) {
        for (k, v) in other.commands {
            self.commands.insert(k, v);
        }
        for (agent, spec) in other.providers {
            let entry = self.providers.entry(agent).or_default();
            if spec.model.is_some() {
                entry.model = spec.model;
            }
            if spec.bin.is_some() {
                entry.bin = spec.bin;
            }
            if spec.transport.is_some() {
                entry.transport = spec.transport;
            }
            if spec.key_env.is_some() {
                entry.key_env = spec.key_env;
            }
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Resolve a word to its canonical command, following aliases. Returns the
    /// canonical name and its spec, or `None` when nothing matches or the
    /// command is disabled.
    pub fn resolve_command(&self, word: &str) -> Option<(&str, &CommandSpec)> {
        let w = word.trim().to_ascii_lowercase();
        for (name, spec) in &self.commands {
            if !spec.enabled {
                continue;
            }
            if name == &w || spec.aliases.iter().any(|a| a.eq_ignore_ascii_case(&w)) {
                return Some((name.as_str(), spec));
            }
        }
        None
    }

    /// The provider spec for an agent, if configured.
    pub fn provider(&self, agent: &str) -> Option<&ProviderSpec> {
        self.providers.get(agent)
    }

    /// Read a dotted key (`providers.jev.model`), for `settings show`/`set`.
    pub fn get(&self, dotted: &str) -> Option<String> {
        let parts: Vec<&str> = dotted.split('.').collect();
        match parts.as_slice() {
            ["providers", agent, field] => {
                let p = self.providers.get(*agent)?;
                match *field {
                    "model" => p.model.clone(),
                    "bin" => p.bin.clone(),
                    "transport" => p.transport.clone(),
                    "key_env" => p.key_env.clone(),
                    _ => None,
                }
            }
            ["commands", cmd, "agent"] => self.commands.get(*cmd)?.agent.clone(),
            _ => None,
        }
    }

    /// Set a dotted key. Only the shapes the settings file defines are
    /// accepted; an unknown key is an error, never a silent no-op.
    pub fn set(&mut self, dotted: &str, value: &str) -> Result<(), String> {
        let parts: Vec<&str> = dotted.split('.').collect();
        match parts.as_slice() {
            ["providers", agent, field] => {
                let p = self.providers.entry((*agent).to_string()).or_default();
                match *field {
                    "model" => p.model = Some(value.to_string()),
                    "bin" => p.bin = Some(value.to_string()),
                    "transport" => p.transport = Some(value.to_string()),
                    "key_env" => p.key_env = Some(value.to_string()),
                    other => return Err(format!("unknown provider field `{other}`")),
                }
                Ok(())
            }
            ["commands", cmd, "agent"] => {
                self.commands.entry((*cmd).to_string()).or_default().agent =
                    Some(value.to_string());
                Ok(())
            }
            ["commands", cmd, "aliases"] => {
                let aliases: Vec<String> = value
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                self.commands.entry((*cmd).to_string()).or_default().aliases = aliases;
                Ok(())
            }
            _ => Err(format!("unknown settings key `{dotted}`")),
        }
    }

    /// One line per command and provider, for `settings show`.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str("COMMANDS\n");
        for (name, spec) in &self.commands {
            out.push_str(&format!(
                "  {name:<14} aliases={:<22} agent={:<10} enabled={}\n",
                if spec.aliases.is_empty() {
                    "-".to_string()
                } else {
                    spec.aliases.join(",")
                },
                spec.agent.as_deref().unwrap_or("-"),
                spec.enabled
            ));
        }
        out.push_str("\nPROVIDERS\n");
        for (agent, p) in &self.providers {
            out.push_str(&format!(
                "  {agent:<10} model={:<34} bin={:<10} transport={:<8} key_env={}\n",
                p.model.as_deref().unwrap_or("-"),
                p.bin.as_deref().unwrap_or("-"),
                p.transport.as_deref().unwrap_or("-"),
                p.key_env.as_deref().unwrap_or("-"),
            ));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_defaults_parse_and_name_the_known_commands() {
        let s = Settings::defaults();
        assert!(s.resolve_command("graph").is_some());
        assert!(s.provider("jev").is_some());
        assert_eq!(
            s.provider("jev").and_then(|p| p.model.clone()),
            Some("oczen/nemotron-3.5-lightning-free".to_string())
        );
    }

    #[test]
    fn user_layer_overrides_only_the_named_fields() {
        let mut s = Settings::defaults();
        let before_bin = s.provider("jev").and_then(|p| p.bin.clone());
        s.merge(
            Settings::parse(
                r#"{"version":1,"providers":{"jev":{"model":"oczen/mimo-v2.5-free"}}}"#,
            )
            .unwrap(),
        );
        let p = s.provider("jev").unwrap();
        assert_eq!(p.model.as_deref(), Some("oczen/mimo-v2.5-free"));
        assert_eq!(p.bin, before_bin, "unnamed fields are untouched");
    }

    #[test]
    fn aliases_resolve_and_disabled_commands_do_not() {
        let mut s = Settings::defaults();
        // `q` is a shipped alias for `query`.
        let (name, _) = s.resolve_command("q").expect("alias resolves");
        assert_eq!(name, "query");

        s.commands.get_mut("query").unwrap().enabled = false;
        assert!(s.resolve_command("q").is_none());
        assert!(s.resolve_command("query").is_none());
    }

    #[test]
    fn set_validates_the_key_and_never_silently_no_ops() {
        let mut s = Settings::defaults();
        s.set("providers.jev.model", "oczen/other").unwrap();
        assert_eq!(s.get("providers.jev.model").as_deref(), Some("oczen/other"));

        s.set("commands.ask.aliases", "q, question ,").unwrap();
        let spec = s.commands.get("ask").unwrap();
        assert_eq!(spec.aliases, vec!["q", "question"]);

        assert!(s.set("providers.jev.nonsense", "x").is_err());
        assert!(s.set("nope", "x").is_err());
    }

    #[test]
    fn keys_are_stored_as_env_names_never_secrets() {
        let mut s = Settings::defaults();
        s.set("providers.jev.key_env", "MY_JEV_KEY").unwrap();
        let text = serde_json::to_string(&s).unwrap();
        assert!(text.contains("MY_JEV_KEY"));
        // The settings document holds no secret-looking value.
        assert!(!text.contains("sk-"));
    }

    #[test]
    fn round_trips_through_json() {
        let s = Settings::defaults();
        let text = serde_json::to_string(&s).unwrap();
        assert_eq!(Settings::parse(&text).unwrap(), s);
        assert!(Settings::parse("{\"version\":9}").is_err());
    }
}
