//! Decision-native agent calls (the Jev pattern).
//!
//! The `@agent` path generates text and parses it back into an `if`. SemIf and
//! OpenJev show the better shape: ask the model for a **typed decision** and
//! read the probabilities directly, so nothing is sampled, nothing is parsed,
//! and an off-schema answer is impossible.
//!
//! This is a client for the Jev-compatible wire API (`POST /v1/systemone`), so
//! it speaks to OpenJev, SemIf, Codiv, or any endpoint that honours the same
//! contract. It is off unless a `jev` provider names an endpoint: no endpoint
//! configured means no decision is made, never a silent fallback to a text
//! model.
//!
//! Call it from a blocking context. `reqwest::blocking` owns a runtime and must
//! not be constructed inside an async task (see the ollama embedder for the
//! same trap).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One typed question. The variants are the Jev contract's three shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    /// Yes/no. `noul` is the contract's name for it.
    Noul {
        instructions: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        criteria: Option<NoulCriteria>,
    },
    /// Pick one of a named set.
    Choice {
        instructions: String,
        criteria: BTreeMap<String, String>,
    },
    /// An ordinal level, returned as an expected value.
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NoulCriteria {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#true: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#false: Option<String>,
}

/// A decision. `confidence` is `1 − H(p)/ln K` per the contract: 1 when the
/// model is certain, 0 when the distribution is uniform.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Answer {
    #[serde(default)]
    pub noul: Option<f64>,
    #[serde(default)]
    pub choice: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub legend: Vec<String>,
    #[serde(default)]
    pub probabilities: serde_json::Value,
    #[serde(default)]
    pub confidence: Option<f64>,
}

impl Answer {
    /// The decision as one line, so a caller can log it without guessing which
    /// variant came back.
    pub fn render(&self) -> String {
        if let Some(n) = self.noul {
            return format!("noul={n:.3}");
        }
        if let Some(c) = &self.choice {
            return format!(
                "choice={c} confidence={:.3}",
                self.confidence.unwrap_or(f64::NAN)
            );
        }
        if let Some(s) = self.score {
            return format!(
                "score={s:.3} confidence={:.3}",
                self.confidence.unwrap_or(f64::NAN)
            );
        }
        "(empty answer)".to_string()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneRequest {
    pub model: String,
    pub state: String,
    pub questions: BTreeMap<String, Question>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SystemOneResponse {
    #[serde(default)]
    pub model: String,
    pub answers: BTreeMap<String, Answer>,
    #[serde(default)]
    pub usage: serde_json::Value,
}

/// A client for a Jev-compatible decision endpoint.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionClient {
    base_url: String,
    api_key: Option<String>,
    model: String,
}

impl DecisionClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: Option<String>,
        model: impl Into<String>,
    ) -> Self {
        Self {
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key,
            model: model.into(),
        }
    }

    /// Build from settings: `providers.jev.endpoint` names the base URL,
    /// `providers.jev.key_env` names the variable holding the key, and
    /// `providers.jev.model` the model id. `None` when no endpoint is set, which
    /// is the default: decision-native calls are opt-in.
    pub fn from_settings() -> Option<Self> {
        let s = crate::settings::Settings::load();
        let p = s.provider("jev")?;
        let endpoint = p.endpoint.clone()?;
        if endpoint.trim().is_empty() {
            return None;
        }
        let key = p
            .key_env
            .as_deref()
            .and_then(|k| std::env::var(k).ok())
            .filter(|v| !v.is_empty());
        Some(Self::new(
            endpoint,
            key,
            p.model
                .clone()
                .unwrap_or_else(|| "openjev-latest".to_string()),
        ))
    }

    pub fn endpoint(&self) -> &str {
        &self.base_url
    }

    pub fn model(&self) -> &str {
        &self.model
    }

    /// Ask the endpoint. A non-2xx, a transport failure, or a body that is not
    /// the contract is an error with the reason — never a guessed answer.
    pub fn decide(
        &self,
        state: &str,
        questions: BTreeMap<String, Question>,
    ) -> Result<SystemOneResponse, String> {
        if questions.is_empty() {
            return Err("no questions asked".to_string());
        }
        let body = SystemOneRequest {
            model: self.model.clone(),
            state: state.to_string(),
            questions,
        };
        let url = format!("{}/v1/systemone", self.base_url);
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| format!("http client: {e}"))?;
        let mut req = client.post(&url).json(&body);
        if let Some(k) = &self.api_key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().map_err(|e| format!("{url}: {e}"))?;
        let status = resp.status();
        let text = resp.text().map_err(|e| format!("{url}: {e}"))?;
        if !status.is_success() {
            return Err(format!("{url} -> HTTP {status}: {}", text.trim()));
        }
        serde_json::from_str::<SystemOneResponse>(&text)
            .map_err(|e| format!("{url}: response is not the system-one contract: {e}"))
    }

    /// Classify one entry into the semiotic grid.
    ///
    /// The grid is a coordinate system, so classification is a decision with a
    /// fixed option set: a `choice` over the domains and a `choice` over the
    /// modes. Reading the probabilities rather than a generated label means the
    /// answer cannot be off-schema, and `confidence` is a number a caller can
    /// threshold on — low-confidence entries go to review instead of being
    /// accepted.
    ///
    /// The domain axis is open (`PHYSIS_DOMAINS`), so the caller passes the
    /// domain list; nothing here hardcodes five.
    pub fn classify_cell(
        &self,
        text: &str,
        domains: &[String],
        modes: &[String],
    ) -> Result<CellDecision, String> {
        if domains.is_empty() || modes.is_empty() {
            return Err("classification needs at least one domain and one mode".to_string());
        }
        let mut questions = BTreeMap::new();
        questions.insert(
            "domain".to_string(),
            Question::Choice {
                instructions: "Which domain is this work about?".to_string(),
                criteria: domains
                    .iter()
                    .map(|d| (d.to_ascii_lowercase(), d.clone()))
                    .collect(),
            },
        );
        questions.insert(
            "mode".to_string(),
            Question::Choice {
                instructions: "Which mode is this work performed in?".to_string(),
                criteria: modes
                    .iter()
                    .map(|m| (m.to_ascii_lowercase(), m.clone()))
                    .collect(),
            },
        );
        let out = self.decide(text, questions)?;
        let domain = out
            .answers
            .get("domain")
            .and_then(|a| a.choice.clone())
            .ok_or_else(|| "the endpoint returned no domain choice".to_string())?;
        let mode = out
            .answers
            .get("mode")
            .and_then(|a| a.choice.clone())
            .ok_or_else(|| "the endpoint returned no mode choice".to_string())?;
        Ok(CellDecision {
            domain: domain.to_ascii_uppercase(),
            domain_confidence: out
                .answers
                .get("domain")
                .and_then(|a| a.confidence)
                .unwrap_or(0.0),
            mode: mode.to_ascii_uppercase(),
            mode_confidence: out
                .answers
                .get("mode")
                .and_then(|a| a.confidence)
                .unwrap_or(0.0),
        })
    }
}

/// A cell assigned by a decision endpoint, with the confidence of each leg.
#[derive(Debug, Clone, PartialEq)]
pub struct CellDecision {
    pub domain: String,
    pub domain_confidence: f64,
    pub mode: String,
    pub mode_confidence: f64,
}

impl CellDecision {
    /// The confidence of the whole cell: the weaker leg. A confident domain with
    /// an unsure mode is not a confident cell, and averaging would hide that.
    pub fn confidence(&self) -> f64 {
        self.domain_confidence.min(self.mode_confidence)
    }

    pub fn cell(&self) -> String {
        format!("{}/{}", self.domain, self.mode)
    }

    /// Whether the cell is confident enough to accept without review.
    pub fn accepted(&self, threshold: f64) -> bool {
        self.confidence() >= threshold
    }

    pub fn render(&self) -> String {
        format!(
            "{} (domain {:.2}, mode {:.2}, cell {:.2})",
            self.cell(),
            self.domain_confidence,
            self.mode_confidence,
            self.confidence()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    /// A one-shot HTTP server that answers with a canned body, so the client is
    /// tested against a real socket rather than a mock.
    fn fixture_server(
        status: &'static str,
        body: &'static str,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let mut buf = [0u8; 8192];
            let n = sock.read(&mut buf).unwrap_or(0);
            let request = String::from_utf8_lossy(&buf[..n]).to_string();
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes());
            let _ = sock.flush();
            request
        });
        (format!("http://{addr}"), handle)
    }

    fn questions() -> BTreeMap<String, Question> {
        let mut q = BTreeMap::new();
        q.insert(
            "billing".to_string(),
            Question::Noul {
                instructions: "Is this a billing issue?".into(),
                criteria: None,
            },
        );
        q.insert(
            "team".to_string(),
            Question::Choice {
                instructions: "Which team?".into(),
                criteria: BTreeMap::from([
                    ("outage".to_string(), "service down".to_string()),
                    ("billing".to_string(), "charges".to_string()),
                ]),
            },
        );
        q
    }

    #[test]
    fn reads_typed_answers_from_the_contract() {
        let body = r#"{"model":"openjev-latest","answers":{
            "billing":{"noul":0.97},
            "team":{"choice":"billing","probabilities":{"outage":0.02,"billing":0.98},"confidence":0.94}},
            "usage":{"input_tokens":42,"output_tokens":0}}"#;
        let (base, handle) = fixture_server("200 OK", body);
        let c = DecisionClient::new(base, Some("k".into()), "openjev-latest");
        let out = c.decide("I was charged twice.", questions()).unwrap();

        assert_eq!(out.answers["billing"].noul, Some(0.97));
        assert_eq!(out.answers["team"].choice.as_deref(), Some("billing"));
        assert_eq!(out.answers["team"].confidence, Some(0.94));
        assert!(out.answers["billing"].render().contains("noul="));

        // The request carried the questions and the state, and never a sampled
        // answer: the client only reads.
        let req = handle.join().unwrap();
        assert!(req.starts_with("POST /v1/systemone"));
        assert!(req.contains("Is this a billing issue?"));
        assert!(req.contains("I was charged twice."));
        assert!(req.contains("Bearer k"));
    }

    #[test]
    fn an_error_status_is_reported_not_guessed() {
        let (base, _h) = fixture_server("429 Too Many Requests", r#"{"detail":"slow down"}"#);
        let c = DecisionClient::new(base, None, "openjev-latest");
        let err = c.decide("s", questions()).unwrap_err();
        assert!(err.contains("HTTP 429"), "{err}");
        assert!(err.contains("slow down"), "{err}");
    }

    #[test]
    fn a_body_off_the_contract_is_reported() {
        let (base, _h) = fixture_server("200 OK", r#"{"not":"the contract"}"#);
        let c = DecisionClient::new(base, None, "openjev-latest");
        let err = c.decide("s", questions()).unwrap_err();
        assert!(err.contains("not the system-one contract"), "{err}");
    }

    #[test]
    fn no_endpoint_configured_means_no_decision_client() {
        // The default settings name no endpoint, so decision-native calls stay
        // off rather than silently falling back to a text model.
        std::env::remove_var("PHYSIS_SETTINGS");
        let s = crate::settings::Settings::defaults();
        assert!(s.provider("jev").and_then(|p| p.endpoint.clone()).is_none());
    }

    #[test]
    fn empty_questions_is_an_error() {
        let (base, _h) = fixture_server("200 OK", "{}");
        let c = DecisionClient::new(base, None, "m");
        assert!(c.decide("s", BTreeMap::new()).is_err());
    }

    #[test]
    fn classifies_a_cell_from_typed_choices_and_gates_on_confidence() {
        let body = r#"{"model":"m","answers":{
            "domain":{"choice":"fabricate","probabilities":{"heal":0.02,"fabricate":0.97},"confidence":0.93},
            "mode":{"choice":"work","probabilities":{"rest":0.3,"work":0.6},"confidence":0.42}}}"#;
        let (base, handle) = fixture_server("200 OK", body);
        let c = DecisionClient::new(base, None, "m");
        let cell = c
            .classify_cell(
                "press seized at the CNC",
                &["HEAL".to_string(), "FABRICATE".to_string()],
                &["REST".to_string(), "WORK".to_string()],
            )
            .unwrap();
        assert_eq!(cell.cell(), "FABRICATE/WORK");
        // The weaker leg is the cell's confidence: a confident domain with an
        // unsure mode is not a confident cell, and averaging would hide that.
        assert_eq!(cell.confidence(), 0.42);
        assert!(cell.accepted(0.4));
        assert!(!cell.accepted(0.5));

        // The request offered a fixed option set, so the answer cannot be
        // off-schema the way a generated label can.
        let req = handle.join().unwrap();
        assert!(req.contains("Which domain"));
        assert!(req.contains("fabricate"));
        assert!(req.contains("press seized at the CNC"));
    }

    #[test]
    fn a_missing_choice_is_reported_not_defaulted() {
        let body =
            r#"{"answers":{"domain":{"noul":0.5},"mode":{"choice":"WORK","confidence":0.9}}}"#;
        let (base, _h) = fixture_server("200 OK", body);
        let c = DecisionClient::new(base, None, "m");
        let err = c
            .classify_cell("x", &["A".to_string()], &["B".to_string()])
            .unwrap_err();
        assert!(err.contains("no domain choice"), "{err}");
    }
}
