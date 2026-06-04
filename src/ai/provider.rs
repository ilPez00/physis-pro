//! LLM provider abstraction with failover cascade.
//!
//! [`Provider`] wraps a single LLM API endpoint (OpenAI-compatible). [`ProviderCascade`]
//! manages multiple providers and routes requests based on task type, availability,
//! and capability (vision, tools, model class).
//!
//! Supports: OpenAI, OpenRouter, Groq, DeepSeek, Gemini, Ollama.

use crate::ai::{AiError, AiResult, tools::ToolDefinition};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

/// Configuration for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Human-readable provider name (e.g. "openai", "groq").
    pub name: String,
    /// Base URL for the OpenAI-compatible API (e.g. `https://api.openai.com/v1`).
    pub base_url: String,
    /// Model identifier (e.g. "gpt-4o-mini", "deepseek-chat").
    pub model: String,
    /// API key for authentication.
    pub api_key: String,
    /// Whether this provider supports vision/image inputs.
    pub supports_vision: bool,
    /// Whether this provider supports function/tool calling.
    pub supports_tools: bool,
    /// Maximum tokens per response.
    pub max_tokens: u32,
    /// HTTP request timeout in seconds.
    pub timeout_secs: f64,
}

/// A message in the chat conversation, following the OpenAI schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: "system", "user", "assistant", or "tool".
    pub role: String,
    /// Message content (text or multi-part).
    pub content: Content,
    /// Tool calls (present on assistant messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallMsg>>,
    /// ID of the tool call this message responds to (present on tool messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Name of the tool (present on tool messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Message content — either a plain text string or a list of parts (text + images).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Plain text content.
    Text(String),
    /// Multi-part content (text + image_url parts).
    Parts(Vec<ContentPart>),
}

/// A single part of a multi-part message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentPart {
    /// Part type: "text" or "image_url".
    #[serde(rename = "type")]
    pub type_: String,
    /// Text content (present when type is "text").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Image URL (present when type is "image_url").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_url: Option<ImageUrl>,
}

/// A URL pointing to an image (data URL or remote URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    /// Image URL (data:image/... or https://...).
    pub url: String,
    /// Detail level: "low" or "high".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// A tool call response from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallMsg {
    /// Unique tool call ID.
    pub id: String,
    /// Always "function".
    #[serde(rename = "type")]
    pub type_: String,
    /// The function to call.
    pub function: FunctionCall,
}

/// A function call specification from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    /// Function/tool name.
    pub name: String,
    /// JSON-encoded arguments.
    pub arguments: String,
}

/// A chat completion request, matching the OpenAI API schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Model identifier.
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Tool definitions (for function calling).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice policy ("auto", "none", or specific tool).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    /// Maximum tokens in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

/// A chat completion response from the API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// List of completion choices.
    pub choices: Vec<Choice>,
    /// Token usage statistics.
    pub usage: Option<Usage>,
}

/// A single completion choice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    /// The response message.
    pub message: ResponseMessage,
    /// Why the generation stopped.
    pub finish_reason: Option<String>,
}

/// The message returned by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMessage {
    /// Assistant role.
    pub role: String,
    /// Response text content.
    #[serde(default)]
    pub content: Option<String>,
    /// Tool calls made by the model.
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCallMsg>>,
}

/// Token usage for a completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    /// Tokens in the prompt.
    pub prompt_tokens: u32,
    /// Tokens in the completion.
    pub completion_tokens: u32,
    /// Total tokens.
    pub total_tokens: u32,
}

/// A single LLM provider backed by an OpenAI-compatible API.
///
/// Wraps configuration, per-provider rate-limiting (cooldown), availability
/// tracking, and HTTP client. Use [`ProviderCascade`] for multi-provider
/// orchestration.
pub struct Provider {
    config: ProviderConfig,
    #[allow(dead_code)]
    key_idx: AtomicI64,
    available: AtomicBool,
    cooldown_until: AtomicI64,
    client: reqwest::Client,
}

impl Provider {
    /// Create a new provider from its configuration.
    pub fn new(config: ProviderConfig) -> Self {
        let timeout = std::time::Duration::from_secs_f64(config.timeout_secs.max(5.0));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            config,
            key_idx: AtomicI64::new(0),
            available: AtomicBool::new(true),
            cooldown_until: AtomicI64::new(0),
            client,
        }
    }

    /// Provider display name.
    pub fn name(&self) -> &str {
        &self.config.name
    }

    /// Model identifier.
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Whether this provider supports function calling.
    pub fn supports_tools(&self) -> bool {
        self.config.supports_tools
    }

    /// Whether the provider is currently available (not in cooldown, not marked unavailable).
    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Relaxed)
            && std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0)
                >= self.cooldown_until.load(Ordering::Relaxed)
    }

    /// Put this provider in cooldown for `seconds` seconds.
    pub fn cooldown(&self, seconds: i64) {
        let until = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64 + seconds)
            .unwrap_or(seconds);
        self.cooldown_until.store(until, Ordering::Relaxed);
    }

    /// Mark this provider as permanently unavailable.
    pub fn mark_unavailable(&self) {
        self.available.store(false, Ordering::Relaxed);
    }

    /// Send a chat completion request to this provider.
    pub async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        max_tokens: Option<u32>,
    ) -> AiResult<ResponseMessage> {
        let mut req = ChatRequest {
            model: self.config.model.clone(),
            messages: messages.to_vec(),
            tools: tools.map(|t| t.to_vec()),
            tool_choice: None,
            max_tokens: max_tokens.or(Some(self.config.max_tokens)),
            temperature: Some(0.7),
        };

        if req.tools.is_some() {
            req.tool_choice = Some(serde_json::json!("auto"));
        }

        let body = serde_json::to_string(&req)?;

        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let mut request = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .body(body);
        if !self.config.api_key.is_empty() {
            request = request.header("Authorization", format!("Bearer {}", self.config.api_key));
        }
        let resp = request.send().await?;

        let status = resp.status();
        if status == 429 {
            self.cooldown(30);
            return Err(AiError::Provider(format!("rate limited on {}", self.config.name)));
        }
        if status == 401 || status == 403 {
            self.mark_unavailable();
            return Err(AiError::Provider(format!("auth error on {}", self.config.name)));
        }
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::Provider(format!(
                "{} returned {}: {}",
                self.config.name,
                status,
                text.chars().take(200).collect::<String>()
            )));
        }

        let cr: ChatResponse = resp.json().await?;
        cr.choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .ok_or_else(|| AiError::Provider("empty response".into()))
    }

    /// Transcribe audio using Whisper.
    pub async fn transcribe(&self, audio_path: &str, model: Option<&str>) -> AiResult<String> {
        let url = format!(
            "{}/audio/transcriptions",
            self.config.base_url.trim_end_matches('/')
        );

        let file_content = std::fs::read(audio_path)?;
        let part = reqwest::multipart::Part::bytes(file_content)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .unwrap();

        let model_name = model.unwrap_or("whisper-large-v3");
        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", model_name.to_string());

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .multipart(form)
            .send()
            .await?;

        if !resp.status().is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(AiError::Provider(format!("transcription failed: {}", text)));
        }

        #[derive(Deserialize)]
        struct TransResponse {
            text: String,
        }

        let tr: TransResponse = resp.json().await?;
        Ok(tr.text)
    }

    /// Translate text to a target language.
    pub async fn translate(&self, text: &str, target_lang: &str) -> AiResult<String> {
        let messages = vec![
            Message {
                role: "system".into(),
                content: Content::Text(format!("Translate the following text to {}. Output ONLY the translated text.", target_lang)),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            Message {
                role: "user".into(),
                content: Content::Text(text.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }
        ];

        let resp = self.complete(&messages, None, None).await?;
        Ok(resp.content.unwrap_or_default())
    }
}

/// Routing hint produced by [`ProviderCascade::classify`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteHint {
    /// Task type: "fast", "smart", or "vision".
    pub task_type: String,
    /// Whether the request includes an image.
    pub has_image: bool,
}

/// A cascade of LLM providers with automatic failover.
///
/// Routes requests to the best available provider based on the task type,
/// provider capabilities, and availability. Providers are tried in order
/// until one succeeds.
pub struct ProviderCascade {
    providers: Vec<Provider>,
}

impl ProviderCascade {
    /// Create an empty cascade.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Add a provider to the cascade.
    pub fn add(&mut self, config: ProviderConfig) {
        self.providers.push(Provider::new(config));
    }

    /// Build a cascade from environment variables.
    ///
    /// Reads: `AI_OPENROUTER_KEY`, `AI_GROQ_KEY`, `AURA_OPENAI_KEY` / `OPENAI_API_KEY`,
    /// `AI_DEEPSEEK_KEY`, `AI_GEMINI_KEY`, plus `AURA_BASE_URL` and `AURA_MODEL`.
    /// Always includes a local Ollama provider at `http://localhost:11434/v1`.
    pub fn from_env() -> Self {
        let mut cascade = Self::new();

        // ── helpers ───────────────────────────────────────────────────────────
        let first_env = |keys: &[&str]| -> Option<String> {
            for k in keys {
                if let Ok(v) = std::env::var(k) {
                    let v = v.trim().trim_matches(|c| c == '\'' || c == '"').to_string();
                    // comma-separated key list → take first
                    let v = v.split(',').next().unwrap_or("").trim().to_string();
                    if !v.is_empty() { return Some(v); }
                }
            }
            None
        };
        // EP_<NAME> env var overrides the default base URL
        let ep = |name: &str, default: &str| -> String {
            let key = format!("EP_{}", name.to_uppercase());
            std::env::var(&key)
                .ok()
                .map(|s| s.trim().trim_end_matches('/').to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| default.to_string())
        };

        // ── Priority 0: local orchestrator ────────────────────────────────────
        if let Some(url) = std::env::var("EP_ORCHESTRATOR").ok().filter(|s| !s.is_empty()) {
            cascade.add(ProviderConfig {
                name: "orchestrator".into(), base_url: url,
                model: "langgraph-orchestrator".into(), api_key: String::new(),
                supports_vision: false, supports_tools: true, max_tokens: 8192, timeout_secs: 60.0,
            });
        }

        // ── Priority 1: Vertex / Gemini ───────────────────────────────────────
        if let Some(key) = first_env(&["VERTEX_API_KEY", "AI_VERTEX_KEY", "AI_GEMINI_KEY"]) {
            cascade.add(ProviderConfig {
                name: "gemini".into(),
                base_url: ep("gemini", "https://generativelanguage.googleapis.com/v1beta/openai"),
                model: "gemini-2.5-flash".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 8192, timeout_secs: 30.0,
            });
        }

        // ── Priority 2: Groq ──────────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_GROQ_KEY", "GROQ_KEY"]) {
            cascade.add(ProviderConfig {
                name: "groq".into(),
                base_url: ep("groq", "https://api.groq.com/openai/v1"),
                model: "llama-3.3-70b-versatile".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 8192, timeout_secs: 15.0,
            });
        }

        // ── Priority 3: OpenAI ────────────────────────────────────────────────
        if let Some(key) = first_env(&["AURA_OPENAI_KEY", "OPENAI_API_KEY"]) {
            let url = std::env::var("AURA_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".into());
            let model = std::env::var("AURA_MODEL").unwrap_or_else(|_| "gpt-4o-mini".into());
            cascade.add(ProviderConfig {
                name: "openai".into(), base_url: url, model, api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 4: DeepSeek ──────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_DEEPSEEK_KEY", "DEEPSEEK_KEY"]) {
            cascade.add(ProviderConfig {
                name: "deepseek".into(),
                base_url: ep("deepseek", "https://api.deepseek.com/v1"),
                model: "deepseek-chat".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 8192, timeout_secs: 30.0,
            });
        }

        // ── Priority 5: OpenRouter ────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_OPENROUTER_KEY", "OPENROUTER_KEY"]) {
            cascade.add(ProviderConfig {
                name: "openrouter".into(),
                base_url: ep("hermes", "https://openrouter.ai/api/v1"),
                model: "deepseek/deepseek-chat".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 6: Mistral ───────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_MISTRAL_KEY", "MISTRAL"]) {
            cascade.add(ProviderConfig {
                name: "mistral".into(),
                base_url: ep("mistral", "https://api.mistral.ai/v1"),
                model: "mistral-large-latest".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 7: Cerebras ──────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_CEREBRAS_KEY", "CEREBRAS_KEY"]) {
            cascade.add(ProviderConfig {
                name: "cerebras".into(),
                base_url: ep("cerebras", "https://api.cerebras.ai/v1"),
                model: "llama3.1-70b".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 8192, timeout_secs: 15.0,
            });
        }

        // ── Priority 8: SambaNova ─────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_SAMBA_NOVA_KEY", "SAMBA_NOVA_KEY"]) {
            cascade.add(ProviderConfig {
                name: "sambanova".into(),
                base_url: ep("samba_nova", "https://api.sambanova.ai/v1"),
                model: "Meta-Llama-3.1-70B-Instruct".into(), api_key: key,
                supports_vision: false, supports_tools: false, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 9: Together ──────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_TOGETHER_KEY", "TOGETHER_KEY"]) {
            cascade.add(ProviderConfig {
                name: "together".into(),
                base_url: ep("together", "https://api.together.xyz/v1"),
                model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 10: xAI / Grok ───────────────────────────────────────────
        if let Some(key) = first_env(&["AI_XAI_KEY", "XAI_KEY"]) {
            cascade.add(ProviderConfig {
                name: "xai".into(),
                base_url: ep("xai", "https://api.x.ai/v1"),
                model: "grok-2".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 11: DeepInfra ────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_DEEPINFRA_KEY", "DEEPINFRA_KEY"]) {
            cascade.add(ProviderConfig {
                name: "deepinfra".into(),
                base_url: ep("deepinfra", "https://api.deepinfra.com/v1"),
                model: "meta-llama/Llama-3.3-70B-Instruct-Turbo".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 12: NVIDIA ───────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_NVIDIA_KEY", "NVIDIA_KEY"]) {
            cascade.add(ProviderConfig {
                name: "nvidia".into(),
                base_url: ep("nvidia", "https://integrate.api.nvidia.com/v1"),
                model: "nvidia/llama-3.1-nemotron-70b-instruct".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 13: Cohere ───────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_COHERE_KEY", "COHERE_KEY"]) {
            cascade.add(ProviderConfig {
                name: "cohere".into(),
                base_url: ep("cohere", "https://api.cohere.com/v2"),
                model: "command-r-plus".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 14: HuggingFace ──────────────────────────────────────────
        if let Some(key) = first_env(&["SVC_HUGGINGFACE_TOKEN", "AI_HUGGINGFACE_KEY", "HUGGINGFACE_KEY"]) {
            cascade.add(ProviderConfig {
                name: "huggingface".into(),
                base_url: ep("huggingface", "https://api-inference.huggingface.co/v1"),
                model: "Qwen/Qwen2.5-72B-Instruct".into(), api_key: key,
                supports_vision: false, supports_tools: false, max_tokens: 2048, timeout_secs: 30.0,
            });
        }

        // ── Priority 15: GitHub Models ────────────────────────────────────────
        if let Some(key) = first_env(&["AI_GITHUB_TOKEN", "GITHUB_TOKEN"]) {
            cascade.add(ProviderConfig {
                name: "github".into(),
                base_url: ep("github", "https://models.inference.ai.azure.com"),
                model: "gpt-4o-mini".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 16: Kluster ──────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_KLUSTER_KEY", "KLUSTER_KEY"]) {
            cascade.add(ProviderConfig {
                name: "kluster".into(),
                base_url: ep("kluster", "https://api.kluster.ai/v1"),
                model: "llama-3.3-70b".into(), api_key: key,
                supports_vision: false, supports_tools: false, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 17: Zhipu ───────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_ZHIPU_KEY", "ZHIPU_KEY"]) {
            cascade.add(ProviderConfig {
                name: "zhipu".into(),
                base_url: ep("zhipu", "https://open.bigmodel.cn/api/paas/v4"),
                model: "glm-4".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 18: BazaarLink ───────────────────────────────────────────
        if let Some(key) = first_env(&["AI_BAZAARLINK_KEY", "BAZAARLINK_KEY"]) {
            cascade.add(ProviderConfig {
                name: "bazaarlink".into(),
                base_url: ep("bazaarlink", "https://api.bazaarlink.ai/v1"),
                model: "gpt-4o".into(), api_key: key,
                supports_vision: true, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 19: AIML API ─────────────────────────────────────────────
        if let Some(key) = first_env(&["AI_AIMLAPI_KEY", "AIMLAPI_KEY"]) {
            cascade.add(ProviderConfig {
                name: "aimlapi".into(),
                base_url: ep("aimlapi", "https://api.aimlapi.com/v2"),
                model: "gpt-4o-mini".into(), api_key: key,
                supports_vision: false, supports_tools: true, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Priority 20: Replicate ────────────────────────────────────────────
        if let Some(key) = first_env(&["SVC_REPLICATE_KEY", "AI_REPLICATE_KEY", "REPLICATE_KEY"]) {
            cascade.add(ProviderConfig {
                name: "replicate".into(),
                base_url: ep("replicate", "https://api.replicate.com/v1"),
                model: "meta/llama-2-70b-chat".into(), api_key: key,
                supports_vision: false, supports_tools: false, max_tokens: 4096, timeout_secs: 30.0,
            });
        }

        // ── Keyless proxies (always added, lowest priority) ───────────────────
        let keyless: &[(&str, &str, &str)] = &[
            ("llm7",         "https://llm7.io/v1",                                               "gpt-4o"),
            ("pollinations",  ep("pollinations", "https://gen.pollinations.ai/v1").leak(),        "openai"),
            ("airforce",      ep("airforce",     "https://api.airforce/v1").leak(),               "gpt-4o"),
            ("g4f",          "https://g4f.space/v1",                                              "gpt-4o"),
            ("g4f_groq",     "https://g4f.space/api/groq",                                        "llama-3.1-8b"),
            ("g4f_gemini",   "https://g4f.space/api/gemini",                                      "gemini-2.5-flash"),
            ("g4f_ollama",   "https://g4f.space/api/ollama",                                      "llama3.1"),
            ("g4f_auto",     "https://g4f.space/api/auto",                                        "auto"),
        ];
        for (name, url, model) in keyless {
            cascade.add(ProviderConfig {
                name: (*name).into(), base_url: (*url).into(), model: (*model).into(),
                api_key: String::new(),
                supports_vision: false, supports_tools: false, max_tokens: 2048, timeout_secs: 20.0,
            });
        }

        // ── Remote public Ollama nodes ────────────────────────────────────────
        let ollama_nodes: &[(&str, &str)] = &[
            ("ollama1", "http://108.181.196.208:11434/v1"),
            ("ollama2", "http://5.149.249.212:11434/v1"),
            ("ollama3", "http://89.111.170.212:11434/v1"),
            ("ollama4", "http://185.211.5.32:11434/v1"),
            ("ollama_local", "http://localhost:11434/v1"),
        ];
        for (name, url) in ollama_nodes {
            cascade.add(ProviderConfig {
                name: (*name).into(), base_url: (*url).into(), model: "llama3.1".into(),
                api_key: String::new(),
                supports_vision: false, supports_tools: false, max_tokens: 4096, timeout_secs: 60.0,
            });
        }

        cascade
    }

    /// List all registered providers with their current availability.
    pub fn providers_list(&self) -> Vec<serde_json::Value> {
        self.providers.iter().map(|p| serde_json::json!({
            "name": p.name(),
            "model": p.model(),
            "available": p.is_available(),
            "supports_vision": p.config.supports_vision,
            "supports_tools": p.config.supports_tools,
            "has_key": !p.config.api_key.is_empty(),
        })).collect()
    }

    /// Classify a request to determine the best routing strategy.
    pub fn classify(&self, query: &str, has_image: bool) -> RouteHint {
        if has_image {
            return RouteHint {
                task_type: "vision".into(),
                has_image: true,
            };
        }
        let q = query.to_lowercase();
        if q.len() < 80 {
            RouteHint { task_type: "fast".into(), has_image: false }
        } else {
            RouteHint { task_type: "smart".into(), has_image: false }
        }
    }

    /// Get available providers matching a route hint.
    pub fn providers_for_task(&self, route: &RouteHint) -> Vec<&Provider> {
        if route.has_image {
            self.providers
                .iter()
                .filter(|p| p.is_available() && p.config.supports_vision)
                .collect()
        } else if route.task_type == "fast" {
            self.providers
                .iter()
                .filter(|p| p.is_available() && !p.config.model.contains("gpt-4"))
                .collect()
        } else {
            self.providers
                .iter()
                .filter(|p| p.is_available())
                .collect()
        }
    }

    /// Send a chat completion through the cascade, trying providers in order.
    pub async fn complete(
        &self,
        messages: &[Message],
        tools: Option<&[ToolDefinition]>,
        route: &RouteHint,
        max_tokens: Option<u32>,
    ) -> AiResult<(ResponseMessage, String)> {
        let candidates = self.providers_for_task(route);

        let needs_tools = tools.map(|t| !t.is_empty()).unwrap_or(false);
        let provider_pool: Vec<&Provider> = if needs_tools {
            candidates
                .into_iter()
                .filter(|p| p.supports_tools())
                .collect()
        } else {
            candidates
        };

        if provider_pool.is_empty() {
            return Err(AiError::NoProvider);
        }

        for provider in &provider_pool {
            match provider.complete(messages, tools, max_tokens).await {
                Ok(resp) => return Ok((resp, provider.name().to_string())),
                Err(e) => {
                    log::warn!("provider {} failed: {:?}", provider.name(), e);
                    continue;
                }
            }
        }

        Err(AiError::NoProvider)
    }

    /// Translate text using the first available provider.
    pub async fn translate(&self, text: &str, target_lang: &str) -> AiResult<String> {
        let best = self.providers.iter().find(|p| p.is_available());
        if let Some(p) = best {
            return p.translate(text, target_lang).await;
        }
        Err(AiError::NoProvider)
    }

    /// Get names of all registered providers.
    pub fn provider_names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name().to_string()).collect()
    }

    /// Transcribe audio using the first available provider (Groq → OpenAI).
    pub async fn transcribe(&self, audio_path: &str, model: Option<&str>) -> AiResult<String> {
        let groq = self.providers.iter().find(|p| p.name() == "groq");
        if let Some(p) = groq {
            return p.transcribe(audio_path, model).await;
        }
        let openai = self.providers.iter().find(|p| p.name() == "openai");
        if let Some(p) = openai {
            return p.transcribe(audio_path, model).await;
        }
        Err(AiError::NoProvider)
    }
}

impl Default for ProviderCascade {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_text_serialization() {
        let msg = Message {
            role: "user".into(),
            content: Content::Text("hello".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("hello"));
        assert!(json.contains("user"));
    }

    #[test]
    fn test_message_parts_serialization() {
        let msg = Message {
            role: "user".into(),
            content: Content::Parts(vec![
                ContentPart {
                    type_: "text".into(),
                    text: Some("describe this".into()),
                    image_url: None,
                },
                ContentPart {
                    type_: "image_url".into(),
                    text: None,
                    image_url: Some(ImageUrl {
                        url: "data:image/jpeg;base64,abc".into(),
                        detail: Some("low".into()),
                    }),
                },
            ]),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("describe this"));
        assert!(json.contains("image_url"));
        assert!(json.contains("data:image/jpeg;base64,abc"));
    }

    #[test]
    fn test_chat_request_serialization() {
        let req = ChatRequest {
            model: "gpt-4".into(),
            messages: vec![Message {
                role: "system".into(),
                content: Content::Text("be helpful".into()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }],
            tools: None,
            tool_choice: None,
            max_tokens: Some(100),
            temperature: Some(0.5),
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("gpt-4"));
        assert!(json.contains("be helpful"));
        assert!(json.contains("100"));
    }

    #[test]
    fn test_provider_new_is_available() {
        let config = ProviderConfig {
            name: "test".into(),
            base_url: "http://localhost".into(),
            model: "test-model".into(),
            api_key: "sk-test".into(),
            supports_vision: false,
            supports_tools: true,
            max_tokens: 1024,
            timeout_secs: 10.0,
        };
        let provider = Provider::new(config);
        assert!(provider.is_available());
        assert_eq!(provider.name(), "test");
        assert_eq!(provider.model(), "test-model");
    }

    #[test]
    fn test_provider_supports_tools() {
        let config = ProviderConfig {
            name: "tooless".into(),
            base_url: "http://localhost".into(),
            model: "no-tools".into(),
            api_key: "sk-nope".into(),
            supports_vision: false,
            supports_tools: false,
            max_tokens: 512,
            timeout_secs: 5.0,
        };
        let provider = Provider::new(config);
        assert!(!provider.supports_tools());
    }

    #[test]
    fn test_cascade_classify_fast() {
        let cascade = ProviderCascade::new();
        let route = cascade.classify("short query", false);
        assert_eq!(route.task_type, "fast");
        assert!(!route.has_image);
    }

    #[test]
    fn test_cascade_classify_long() {
        let cascade = ProviderCascade::new();
        let long = "a".repeat(100);
        let route = cascade.classify(&long, false);
        assert_eq!(route.task_type, "smart");
    }

    #[test]
    fn test_cascade_classify_vision() {
        let cascade = ProviderCascade::new();
        let route = cascade.classify("anything", true);
        assert_eq!(route.task_type, "vision");
        assert!(route.has_image);
    }

    #[test]
    fn test_cascade_empty_providers_for_task() {
        let cascade = ProviderCascade::new();
        let route = cascade.classify("test", false);
        let providers = cascade.providers_for_task(&route);
        assert!(providers.is_empty());
    }

    #[test]
    fn test_cascade_provider_names_empty() {
        let cascade = ProviderCascade::new();
        let names = cascade.provider_names();
        assert!(names.is_empty());
    }

    #[test]
    fn test_cascade_default_is_empty() {
        let cascade = ProviderCascade::default();
        let names = cascade.provider_names();
        assert!(names.is_empty());
    }
}
