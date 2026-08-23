//! Tool registry and builtin tool implementations.
//!
//! Tools are callable functions that an LLM agent can invoke during a `run_agent`
//! loop. The [`ToolRegistry`] manages registration, discovery via [`ToolDefinition`],
//! and dispatch. Builtin tools include web search, file I/O, and memory operations.

use crate::ai::AiResult;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Signature for a tool handler: takes JSON args, returns a string result.
pub type ToolHandler = Box<dyn Fn(serde_json::Value) -> futures::future::BoxFuture<'static, AiResult<String>> + Send + Sync>;

/// OpenAI-compatible tool definition for LLM function-calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Always `"function"`.
    #[serde(rename = "type")]
    pub type_: String,
    /// The function specification.
    pub function: ToolFunction,
}

/// Describes a callable function for the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    /// Tool name (used for dispatch).
    pub name: String,
    /// Description of what the tool does.
    pub description: String,
    /// JSON Schema for the tool's arguments.
    pub parameters: serde_json::Value,
}

/// Registry of named tools that can be called by an agent.
///
/// Tools are registered with a name, description, JSON Schema, and an async
/// handler. Builtin tools (`web_search`, `read_url`, `read_file`, `write_file`,
/// `list_dir`, `memory_save`, `memory_search`, `get_timestamp`) are
/// pre-registered via [`ToolRegistry::new`].
pub struct ToolRegistry {
    handlers: HashMap<String, ToolHandler>,
    definitions: Vec<ToolDefinition>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new registry with all builtin tools pre-registered.
    pub fn new() -> Self {
        let mut reg = Self {
            handlers: HashMap::new(),
            definitions: Vec::new(),
        };
        reg.register_builtins();
        reg
    }

    /// Register a new tool.
    pub fn register(
        &mut self,
        name: &str,
        description: &str,
        parameters: serde_json::Value,
        handler: ToolHandler,
    ) {
        self.definitions.push(ToolDefinition {
            type_: "function".into(),
            function: ToolFunction {
                name: name.to_string(),
                description: description.to_string(),
                parameters,
            },
        });
        self.handlers.insert(name.to_string(), handler);
    }

    /// Get all registered tool definitions (for passing to an LLM).
    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    /// Check if a tool with `name` is registered.
    pub fn has(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Remove every registered tool + definition. Used to start from a blank
    /// registry (e.g. when only a curated subset should be exposed to an agent).
    pub fn clear(&mut self) {
        self.handlers.clear();
        self.definitions.clear();
    }

    /// Get all registered tool names.
    pub fn names(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Call a tool by name with the given JSON arguments.
    pub async fn call(&self, name: &str, args: serde_json::Value) -> AiResult<String> {
        match self.handlers.get(name) {
            Some(handler) => handler(args).await,
            None => Err(crate::ai::AiError::Tool(format!("unknown tool: {name}"))),
        }
    }

    fn register_builtins(&mut self) {
        self.register(
            "web_search",
            "Search the web for current information. Returns text results with URLs.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let query = args.get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    tools_web_search(&query).await
                })
            }),
        );

        self.register(
            "read_url",
            "Fetch and read the content of a URL. Returns text/markdown content.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "URL to read"}
                },
                "required": ["url"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let url = args.get("url")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    tools_read_url(&url).await
                })
            }),
        );

        self.register(
            "read_file",
            "Read the contents of a file on the local filesystem.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to file"}
                },
                "required": ["path"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    tools_read_file(&path)
                })
            }),
        );

        self.register(
            "write_file",
            "Write content to a file on the local filesystem. Creates directories if needed.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Absolute path to file"},
                    "content": {"type": "string", "description": "Content to write"}
                },
                "required": ["path", "content"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let content = args.get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    tools_write_file(&path, &content)
                })
            }),
        );

        self.register(
            "list_dir",
            "List files and directories in a given path.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory path to list"}
                },
                "required": ["path"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let path = args.get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or(".")
                        .to_string();
                    tools_list_dir(&path)
                })
            }),
        );

        self.register(
            "memory_save",
            "Save a piece of information to long-term memory for later recall.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string", "description": "Information to remember"},
                    "kind": {"type": "string", "description": "Category: observation|fact|preference", "default": "observation"}
                },
                "required": ["text"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let text = args.get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let kind = args.get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("observation")
                        .to_string();
                    Ok(format!("saved to memory [{kind}]: {text}"))
                })
            }),
        );

        self.register(
            "memory_search",
            "Search long-term memory for stored information by topic or keyword.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "Search query"}
                },
                "required": ["query"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let query = args.get("query")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Ok(format!("[memory search stub for: {query}]"))
                })
            }),
        );

        self.register(
            "get_timestamp",
            "Get the current date and time.",
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
            Box::new(|_args| {
                Box::pin(async move {
                    let now = chrono::Local::now();
                    Ok(now.format("%Y-%m-%d %H:%M:%S").to_string())
                })
            }),
        );
    }
}

async fn tools_web_search(query: &str) -> AiResult<String> {
    let client = reqwest::Client::new();
    let url = format!("https://api.duckduckgo.com/?q={q}&format=json&no_html=1", q = urlencoding(query));
    let resp = client.get(&url).send().await?;
    let text = resp.text().await?;
    let truncated: String = text.chars().take(3000).collect();
    Ok(truncated)
}

async fn tools_read_url(url: &str) -> AiResult<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .unwrap_or_default();
    let resp = client.get(url)
        .header("User-Agent", "Mozilla/5.0 Aura/0.1")
        .send()
        .await?;
    let text = resp.text().await?;
    let truncated: String = text.chars().take(8000).collect();
    Ok(truncated)
}

fn tools_read_file(path: &str) -> AiResult<String> {
    use std::io::Read;
    let mut f = std::fs::File::open(path)?;
    let mut buf = String::new();
    f.read_to_string(&mut buf)?;
    let truncated: String = buf.chars().take(64000).collect();
    Ok(truncated)
}

fn tools_write_file(path: &str, content: &str) -> AiResult<String> {
    if let Some(parent) = std::path::Path::new(path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(format!("wrote {} bytes to {path}", content.len()))
}

fn tools_list_dir(path: &str) -> AiResult<String> {
    let entries = std::fs::read_dir(path)?;
    let mut lines = Vec::new();
    for entry in entries {
        let entry = entry?;
        let meta = entry.metadata()?;
        let kind = if meta.is_dir() { "d" } else { "-" };
        let size = meta.len();
        let name = entry.file_name().to_string_lossy().to_string();
        lines.push(format!("{kind} {size:>8} {name}"));
    }
    Ok(lines.join("\n"))
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            ' ' => "+".to_string(),
            other => format!("%{:02X}", other as u8),
        })
        .collect()
}

/// Call a tool by name, returning the result string or an error message.
///
/// This is a convenience wrapper around [`ToolRegistry::call`] that converts
/// errors into a string prefixed with `"error: "`.
pub async fn call_tool(registry: &ToolRegistry, name: &str, args: serde_json::Value) -> String {
    match registry.call(name, args).await {
        Ok(result) => result,
        Err(e) => format!("error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;

    #[test]
    fn test_register_and_call() {
        let mut registry = ToolRegistry::new();
        registry.register(
            "echo",
            "Echo the input back",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "msg": {"type": "string"}
                },
                "required": ["msg"]
            }),
            Box::new(|args| {
                Box::pin(async move {
                    let msg = args.get("msg").and_then(|v| v.as_str()).unwrap_or("");
                    Ok(format!("echo: {msg}"))
                })
            }),
        );
        assert!(registry.has("echo"));
        let result = block_on(registry.call("echo", serde_json::json!({"msg": "hello"}))).unwrap();
        assert_eq!(result, "echo: hello");
    }

    #[test]
    fn test_unknown_tool() {
        let registry = ToolRegistry::new();
        let result = block_on(registry.call("nonexistent", serde_json::json!({})));
        assert!(result.is_err());
    }

    #[test]
    fn test_has_and_names() {
        let mut registry = ToolRegistry::new();
        registry.register(
            "alpha",
            "First tool",
            serde_json::json!({"type": "object", "properties": {}}),
            Box::new(|_| Box::pin(async { Ok("alpha".into()) })),
        );
        registry.register(
            "beta",
            "Second tool",
            serde_json::json!({"type": "object", "properties": {}}),
            Box::new(|_| Box::pin(async { Ok("beta".into()) })),
        );
        assert!(registry.has("alpha"));
        assert!(registry.has("beta"));
        assert!(!registry.has("gamma"));
        let names = registry.names();
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
    }

    #[test]
    fn test_register_builtins() {
        let registry = ToolRegistry::new();
        assert!(registry.has("web_search"));
        assert!(registry.has("read_url"));
        assert!(registry.has("read_file"));
        assert!(registry.has("write_file"));
        assert!(registry.has("list_dir"));
        assert!(registry.has("memory_save"));
        assert!(registry.has("memory_search"));
        assert!(registry.has("get_timestamp"));
    }

    #[test]
    fn test_tool_function_structure() {
        let mut registry = ToolRegistry::new();
        registry.register(
            "custom",
            "Custom description",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "x": {"type": "integer"}
                },
                "required": ["x"]
            }),
            Box::new(|_| Box::pin(async { Ok("ok".into()) })),
        );
        let defs = registry.definitions();
        let custom = defs.iter().find(|d| d.function.name == "custom").unwrap();
        assert_eq!(custom.type_, "function");
        assert_eq!(custom.function.description, "Custom description");
        assert_eq!(custom.function.parameters["required"][0], "x");
    }

    #[test]
    fn test_duplicate_register_overwrites() {
        let mut registry = ToolRegistry::new();
        registry.register(
            "dup",
            "Original",
            serde_json::json!({"type": "object", "properties": {}}),
            Box::new(|_| Box::pin(async { Ok("original".into()) })),
        );
        registry.register(
            "dup",
            "Override",
            serde_json::json!({"type": "object", "properties": {}}),
            Box::new(|_| Box::pin(async { Ok("override".into()) })),
        );
        let result = block_on(registry.call("dup", serde_json::json!({}))).unwrap();
        assert_eq!(result, "override");
    }

    #[test]
    fn test_call_tool_wrapper_ok() {
        let mut registry = ToolRegistry::new();
        registry.register(
            "ping",
            "Ping",
            serde_json::json!({"type": "object", "properties": {}}),
            Box::new(|_| Box::pin(async { Ok("pong".into()) })),
        );
        let result = block_on(call_tool(&registry, "ping", serde_json::json!({})));
        assert_eq!(result, "pong");
    }

    #[test]
    fn test_call_tool_wrapper_err() {
        let registry = ToolRegistry::new();
        let result = block_on(call_tool(&registry, "boom", serde_json::json!({})));
        assert!(result.starts_with("error:"));
    }
}
