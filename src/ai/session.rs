use crate::ai::agent::{run_agent, AgentConfig};
use crate::ai::provider::{Content, ContentPart, ImageUrl, Message, ProviderCascade};
use crate::ai::tools::ToolRegistry;
use crate::ai::AiResult;

const MAX_HISTORY: usize = 30;

pub struct Session {
    provider_cascade: ProviderCascade,
    tools: ToolRegistry,
    config: AgentConfig,
    history: Vec<Message>,
}

impl Session {
    pub fn new(system_prompt: &str) -> Self {
        let cascade = ProviderCascade::from_env();
        let providers = cascade.provider_names();
        log::info!("session created with providers: {}", providers.join(", "));

        Self {
            provider_cascade: cascade,
            tools: ToolRegistry::new(),
            config: AgentConfig {
                system_prompt: system_prompt.to_string(),
                max_tokens: 4096,
                detail: "low".into(),
                ..Default::default()
            },
            history: Vec::new(),
        }
    }

    pub fn provider_cascade(&self) -> &ProviderCascade {
        &self.provider_cascade
    }

    pub fn tools(&self) -> &ToolRegistry {
        &self.tools
    }

    pub fn config(&self) -> &AgentConfig {
        &self.config
    }

    pub fn history(&self) -> &[Message] {
        &self.history
    }

    pub fn push_context(&mut self, text: &str, frame_b64: Option<&str>, mode: &str) {
        let mut parts = vec![ContentPart {
            type_: "text".into(),
            text: Some(format!("[{mode} context] {text}")),
            image_url: None,
        }];
        if let Some(b64) = frame_b64 {
            parts.push(ContentPart {
                type_: "image_url".into(),
                text: None,
                image_url: Some(ImageUrl {
                    url: format!("data:image/jpeg;base64,{b64}"),
                    detail: Some("low".into()),
                }),
            });
        }
        self.history.push(Message {
            role: "user".into(),
            content: Content::Parts(parts),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        self.trim_history();
    }

    pub async fn query(&mut self, question: &str, mode: &str) -> AiResult<String> {
        let output = run_agent(
            &self.provider_cascade,
            &self.tools,
            &self.config,
            &self.history,
            question,
            None,
            mode,
            None,
        )
        .await?;

        self.history.push(Message {
            role: "user".into(),
            content: Content::Text(question.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        self.history.push(Message {
            role: "assistant".into(),
            content: Content::Text(output.text.clone()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        self.trim_history();

        Ok(output.text)
    }

    pub async fn query_with_frame(
        &mut self,
        question: &str,
        frame_b64: &str,
        mode: &str,
    ) -> AiResult<String> {
        let output = run_agent(
            &self.provider_cascade,
            &self.tools,
            &self.config,
            &self.history,
            question,
            Some(frame_b64),
            mode,
            None,
        )
        .await?;

        self.history.push(Message {
            role: "user".into(),
            content: Content::Parts(vec![
                ContentPart {
                    type_: "text".into(),
                    text: Some(question.to_string()),
                    image_url: None,
                },
                ContentPart {
                    type_: "image_url".into(),
                    text: None,
                    image_url: Some(ImageUrl {
                        url: format!("data:image/jpeg;base64,{frame_b64}"),
                        detail: Some("low".into()),
                    }),
                },
            ]),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        self.history.push(Message {
            role: "assistant".into(),
            content: Content::Text(output.text.clone()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        self.trim_history();

        Ok(output.text)
    }

    pub async fn auto_observe(&mut self, mode: &str) -> AiResult<Option<String>> {
        if self.history.is_empty() {
            return Ok(None);
        }

        let observe_config = AgentConfig {
            system_prompt: self.config.system_prompt.clone(),
            max_tokens: 200,
            detail: "low".into(),
            ..Default::default()
        };

        let output = run_agent(
            &self.provider_cascade,
            &self.tools,
            &observe_config,
            &[],
            "Brief observation of current context. 1-2 sentences.",
            None,
            mode,
            None,
        )
        .await?;

        Ok(Some(output.text))
    }

    pub fn provider_names(&self) -> Vec<String> {
        self.provider_cascade.provider_names()
    }

    pub fn tool_names(&self) -> Vec<String> {
        self.tools.names()
    }

    fn trim_history(&mut self) {
        let sys_count = self.history.iter().filter(|m| m.role == "system").count();
        if self.history.len() > MAX_HISTORY + sys_count {
            let all: Vec<Message> = self.history.drain(..).collect();
            let systems: Vec<Message> =
                all.iter().filter(|m| m.role == "system").cloned().collect();
            let non_systems: Vec<Message> =
                all.into_iter().filter(|m| m.role != "system").collect();
            let skip = non_systems.len().saturating_sub(MAX_HISTORY);
            let keep: Vec<Message> = non_systems.into_iter().skip(skip).collect();
            let mut combined = systems;
            combined.extend(keep);
            self.history = combined;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new_defaults() {
        let s = Session::new("You are Physis.");
        assert_eq!(s.config.system_prompt, "You are Physis.");
        assert_eq!(s.config.max_tokens, 4096);
        assert!(s.history.is_empty());
    }

    #[test]
    fn test_push_context_text_only() {
        let mut s = Session::new("test");
        s.push_context("some context here", None, "COHERENCE");
        assert_eq!(s.history.len(), 1);
        assert_eq!(s.history[0].role, "user");
        match &s.history[0].content {
            Content::Parts(parts) => {
                assert_eq!(parts.len(), 1);
                assert_eq!(parts[0].type_, "text");
                assert!(parts[0]
                    .text
                    .as_ref()
                    .unwrap()
                    .contains("some context here"));
            }
            _ => panic!("expected Content::Parts"),
        }
    }

    #[test]
    fn test_push_context_with_image() {
        let mut s = Session::new("test");
        s.push_context("what is this", Some("base64data=="), "DATA");
        assert_eq!(s.history.len(), 1);
        match &s.history[0].content {
            Content::Parts(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[1].type_, "image_url");
                assert!(parts[1]
                    .image_url
                    .as_ref()
                    .unwrap()
                    .url
                    .contains("base64data=="));
            }
            _ => panic!("expected Content::Parts"),
        }
    }

    #[test]
    fn test_provider_names_never_panics() {
        let s = Session::new("test");
        let _names = s.provider_names();
    }

    #[test]
    fn test_tool_names_has_builtins() {
        let s = Session::new("test");
        assert!(
            !s.tool_names().is_empty(),
            "ToolRegistry has built-in tools"
        );
    }

    #[test]
    fn test_trim_history_below_max() {
        let mut s = Session::new("test");
        for i in 0..20 {
            s.push_context(&format!("ctx {i}"), None, "COHERENCE");
        }
        assert_eq!(s.history.len(), 20);
    }

    #[test]
    fn test_trim_history_trims_past_max() {
        let mut s = Session::new("test");
        let overflow = MAX_HISTORY + 10;
        for i in 0..overflow {
            s.push_context(&format!("ctx {i}"), None, "COHERENCE");
        }
        assert_eq!(
            s.history.len(),
            MAX_HISTORY,
            "history must be trimmed to MAX_HISTORY"
        );
    }

    #[test]
    fn test_trim_history_preserves_system_messages() {
        let mut s = Session::new("test");
        s.history.push(Message {
            role: "system".into(),
            content: Content::Text("sys".into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });
        for i in 0..MAX_HISTORY + 5 {
            s.push_context(&format!("ctx {i}"), None, "COHERENCE");
        }
        assert!(
            s.history.iter().any(|m| m.role == "system"),
            "system messages must survive trim"
        );
    }

    #[test]
    fn test_trim_history_no_crash_empty() {
        let mut s = Session::new("test");
        s.trim_history();
        assert!(s.history.is_empty());
    }

    #[test]
    fn test_trim_history_removes_oldest_non_system() {
        let mut s = Session::new("test");
        for i in 0..MAX_HISTORY + 10 {
            s.push_context(&format!("ctx {i}"), None, "COHERENCE");
        }
        let texts: Vec<&str> = s
            .history
            .iter()
            .filter_map(|m| match &m.content {
                Content::Parts(parts) => parts.first().and_then(|p| p.text.as_deref()),
                Content::Text(t) => Some(t.as_str()),
            })
            .collect();
        assert!(
            texts.iter().any(|t| t.contains("ctx 30")),
            "should contain latest entries"
        );
        assert!(
            !texts.iter().any(|t| t.contains("ctx 0")),
            "oldest entries should be trimmed"
        );
    }
}
