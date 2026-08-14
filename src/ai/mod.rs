//! AI provider cascade, agent loop, tool registry, and episodic memory.
//!
//! This module provides the AI mediation layer for Physis:
//! - [`provider::ProviderCascade`] manages multiple LLM providers with failover
//! - [`agent::run_agent`] orchestrates a tool-using agent loop
//! - [`tools::ToolRegistry`] registers and invokes builtin tools
//! - [`memory::EpisodicMemory`] persists agent memories via sled
//! - [`session::Session`] manages a conversational context

pub mod agent;
pub mod memory;
pub mod provider;
pub mod session;
pub mod tools;
pub mod onnx_worker;

/// Result type for all AI operations.
pub type AiResult<T> = Result<T, AiError>;

/// Errors that can occur in the AI subsystem.
#[derive(Debug, thiserror::Error)]
pub enum AiError {
    /// An LLM provider returned an error (rate limit, auth, timeout).
    #[error("Provider error: {0}")]
    Provider(String),
    /// A tool invocation failed.
    #[error("Tool error: {0}")]
    Tool(String),
    /// JSON serialization or deserialization error.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Session-level error.
    #[error("Session error: {0}")]
    Session(String),
    /// Filesystem I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Sled database error.
    #[error("Sled error: {0}")]
    Sled(#[from] sled::Error),
    /// Agent exceeded maximum tool call rounds.
    #[error("Max tool rounds reached")]
    MaxToolRounds,
    /// No LLM provider is available or configured.
    #[error("No provider available")]
    NoProvider,
}
