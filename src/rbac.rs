//! Minimal role-based access control for the web API.
//!
//! A `TokenTable` maps bearer secrets to roles. Tokens are issued as
//! `role:secret` (e.g. `editor:9f3c...`) so a buyer can hand a contractor a
//! read-only viewer token and an operator an editor token, without sharing the
//! admin secret. Replaces the old single flat `PHYSIS_API_TOKEN`.
//!
//! Enforcement lives in `web_server.rs`'s `require_auth` middleware: it reads
//! the request method + path to pick the minimum required role, then checks the
//! presented bearer token against the table (constant-time compare).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Three-tier access model. `Viewer` < `Editor` < `Admin` (ordered by `ord`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Read-only: GETs across the API.
    Viewer,
    /// Mutating: POSTs that change session/graph state.
    Editor,
    /// Privileged: config reload, axis promotion, token/role changes.
    Admin,
}

impl Role {
    /// Human-readable label for logs/UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Viewer => "viewer",
            Role::Editor => "editor",
            Role::Admin => "admin",
        }
    }

    /// Parse a `role:secret` token string into (Role, secret). Returns None on
    /// an unknown role name — never panics on a malformed token.
    pub fn parse_token(token: &str) -> Option<(Role, &str)> {
        let (role, secret) = token.split_once(':')?;
        let role = match role.trim().to_ascii_lowercase().as_str() {
            "viewer" => Role::Viewer,
            "editor" => Role::Editor,
            "admin" => Role::Admin,
            _ => return None,
        };
        Some((role, secret))
    }
}

/// A set of issued tokens and their roles. Cheap to clone (Arc-shared).
#[derive(Debug, Clone, Default)]
pub struct TokenTable {
    /// secret -> role
    inner: HashMap<String, Role>,
}

impl TokenTable {
    /// Empty table (no auth required — loopback dev mode).
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `role:secret` token. Returns false if the token string is
    /// malformed (unknown role or missing `:`).
    pub fn register(&mut self, token: &str) -> bool {
        match Role::parse_token(token) {
            Some((role, secret)) if !secret.is_empty() => {
                self.inner.insert(secret.to_string(), role);
                true
            }
            _ => false,
        }
    }

    /// Register many tokens; ignores (but tolerates) malformed ones.
    pub fn extend(&mut self, tokens: &[String]) {
        for t in tokens {
            self.register(t);
        }
    }

    /// True when at least one token is configured (auth is enforced).
    pub fn is_enforced(&self) -> bool {
        !self.inner.is_empty()
    }

    /// Constant-time check: does `presented` (the bearer secret) map to a role
    /// at least `min`? Unknown secrets return None.
    pub fn authenticate(&self, presented: &str) -> Option<Role> {
        self.inner.get(presented).copied()
    }
}

/// Resolve the minimum role required for a request, by method + path.
/// GET/HEAD -> Viewer; mutating POST/PUT/DELETE -> Editor; the few privileged
/// paths (config reload, axis promotion, token admin) -> Admin.
pub fn min_role_for(method: &str, path: &str) -> Role {
    // Privileged endpoints.
    if path.starts_with("/api/v1/config") || path.starts_with("/api/v1/axes/promote") {
        return Role::Admin;
    }
    match method.to_ascii_uppercase().as_str() {
        "GET" | "HEAD" | "OPTIONS" => Role::Viewer,
        _ => Role::Editor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_ordering() {
        assert!(Role::Viewer < Role::Editor);
        assert!(Role::Editor < Role::Admin);
        assert_ne!(Role::Viewer, Role::Editor);
    }

    #[test]
    fn test_parse_token() {
        assert_eq!(Role::parse_token("editor:abc"), Some((Role::Editor, "abc")));
        assert_eq!(Role::parse_token("ADMIN:xyz"), Some((Role::Admin, "xyz")));
        assert_eq!(Role::parse_token("viewer:"), Some((Role::Viewer, ""))); // lenient; register() rejects empty secret
        assert_eq!(Role::parse_token("wizard:abc"), None); // unknown role
        assert_eq!(Role::parse_token("no-colon"), None);
    }

    #[test]
    fn test_token_table_register_and_auth() {
        let mut t = TokenTable::new();
        assert!(!t.is_enforced());
        assert!(t.register("viewer:read123"));
        assert!(t.register("editor:write456"));
        assert!(!t.register("bogus:xxx")); // unknown role rejected
        assert!(t.is_enforced());
        assert_eq!(t.authenticate("read123"), Some(Role::Viewer));
        assert_eq!(t.authenticate("write456"), Some(Role::Editor));
        assert_eq!(t.authenticate("nope"), None);
    }

    #[test]
    fn test_min_role_for() {
        assert_eq!(min_role_for("GET", "/api/v1/ontology/list"), Role::Viewer);
        assert_eq!(min_role_for("POST", "/api/v1/goals"), Role::Editor);
        assert_eq!(min_role_for("POST", "/api/v1/config/reload"), Role::Admin);
        assert_eq!(min_role_for("POST", "/api/v1/axes/promote"), Role::Admin);
    }
}
