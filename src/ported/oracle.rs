// provenance: the OpenAI-compatible transport of physis-core src/oracle.rs
// (core HEAD d65858b) moved here by the 2026-09-14 Core/Product split.
// Core keeps the oracle *types* and the deterministic comparison
// (`physis_core::oracle`); this module keeps the *call*.

//! Big-model oracle leg — the provider transport.
//!
//! A substantially larger model is used as **reference/oracle**, never as part
//! of the deployed system. Provider integration is product work: Core reports
//! the leg (`not_configured` / `failed`) and carries on measuring; the call
//! lives here, next to the rest of the product's model plumbing.
//!
//! Env (same names Core still resolves for its reporting):
//!
//! ```text
//! PHYSIS_ORACLE_URL    (default https://openrouter.ai/api/v1)
//! PHYSIS_ORACLE_KEY    (fallback: OPENROUTER_API_KEY, then GROQ_API_KEY)
//! PHYSIS_ORACLE_MODEL  (default openai/gpt-4o)
//! ```

use physis_core::oracle::OracleConfig;

/// One OpenAI-compatible chat call. `max_tokens` is tight: we buy reference
/// behaviour, not essays.
pub fn complete(
    cfg: &OracleConfig,
    key: &str,
    system: &str,
    prompt: &str,
    max_tokens: u32,
) -> anyhow::Result<(String, f64)> {
    let started = std::time::Instant::now();
    let body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0,
        "max_tokens": max_tokens,
        "messages": [
            {"role": "system", "content": system},
            {"role": "user", "content": prompt},
        ],
    });
    let url = format!("{}/chat/completions", cfg.url.trim_end_matches('/'));
    let resp = reqwest::blocking::Client::new()
        .post(&url)
        .header("Authorization", format!("Bearer {key}"))
        .header("HTTP-Referer", "https://github.com/ilPez00/physis-pro")
        .header("X-Title", "physis oracle benchmark")
        .json(&body)
        .send()
        .map_err(|e| anyhow::anyhow!("oracle request: {e}"))?;
    let text = resp
        .text()
        .map_err(|e| anyhow::anyhow!("oracle body: {e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("oracle json: {e}: {text}"))?;
    let content = v
        .pointer("/choices/0/message/content")
        .and_then(|c| c.as_str())
        .ok_or_else(|| anyhow::anyhow!("oracle shape: {text}"))?
        .to_string();
    Ok((content, started.elapsed().as_secs_f32() as f64 * 1000.0))
}

/// Are we configured to call an oracle at all? Same resolution Core uses for
/// reporting, exposed here so the product can decide to run the leg.
pub fn configured() -> Option<(OracleConfig, String)> {
    physis_core::oracle::from_env()
}
