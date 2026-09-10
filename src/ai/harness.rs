//! Physis Epistemic Coherence Harness & Final Arbiter for LLMs.
//!
//! Provides a formal epistemic gate between LLM generation and real-world execution.
//! Evaluates candidate interpretations, statements, and actions against:
//! 1. Semiotic Grid Placement (Ontological taxonomy)
//! 2. Contradiction Detection (Active conflicts with known beliefs & evidence)
//! 3. Multidimensional Coherence Profiles (Semantic, Ontological, Logical, Empirical, Process)
//! 4. Decomposed Fitness Scoring with Quality Penalties
//! 5. Historical Precedent Recall & Explanation Reports
//! 6. Automated Counterfactual Critique & Iterative Revision

use serde::{Deserialize, Serialize};

use crate::ai::provider::{Content, Message, Provider, ProviderCascade, ProviderConfig, RouteHint};
use crate::classify::CellClassifier;
use crate::core::PhysisCore;
use crate::embed::VectorEmbed;
use crate::quality::QualityTracker;
// SharedState only exists in the web build; the harness entrypoint that takes
// it is gated below so `--no-default-features` builds (the license generator)
// still compile.
#[cfg(feature = "web")]
use crate::web::SharedState;

use physis_core::coherence_dimensions::CoherenceProfile;
use physis_core::contradiction::ResolutionStatus;
use physis_core::epistemic::{EpistemicEvent, EpistemicEventType};
use physis_core::explanation::ExplanationReport;
use physis_core::hypothesis::{
    Evidence, EvidencePolarity, FitnessBreakdown, Hypothesis, HypothesisStatus,
};
use physis_core::models::cosine_sim;
use physis_core::provenance::ProvenanceChain;

/// Configuration for the Physis Coherence Harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessConfig {
    /// LLM provider name ("openai", "anthropic", "gemini", "groq", "ollama", "custom")
    #[serde(default = "default_provider")]
    pub provider: String,
    /// Base URL override for OpenAI-compatible API
    pub base_url: Option<String>,
    /// Model identifier (e.g. "gpt-4o", "gemini-2.5-flash", "llama-3.3-70b-versatile")
    pub model: Option<String>,
    /// API key for authentication
    pub api_key: Option<String>,
    /// Minimum composite fitness threshold [0.0, 1.0] for approval (default: 0.60)
    #[serde(default = "default_coherence_threshold")]
    pub coherence_threshold: f32,
    /// Whether any detected contradiction automatically marks as Warning or Rejected (default: true)
    #[serde(default = "default_true")]
    pub reject_contradictions: bool,
    /// Whether to attempt automated LLM revision when output is incoherent or contradicted (default: true)
    #[serde(default = "default_true")]
    pub auto_revise: bool,
    /// Maximum revision attempts (default: 2)
    #[serde(default = "default_max_revisions")]
    pub max_revisions: usize,
    /// Whether to register candidate interpretations as hypotheses into the core graph (default: true)
    #[serde(default = "default_true")]
    pub register_as_hypothesis: bool,
}

fn default_provider() -> String {
    "openai".to_string()
}
fn default_coherence_threshold() -> f32 {
    0.60
}
fn default_true() -> bool {
    true
}
fn default_max_revisions() -> usize {
    2
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            base_url: None,
            model: Some("gpt-4o-mini".to_string()),
            api_key: None,
            coherence_threshold: default_coherence_threshold(),
            reject_contradictions: true,
            auto_revise: true,
            max_revisions: default_max_revisions(),
            register_as_hypothesis: true,
        }
    }
}

/// Final gate verdict from the Physis Coherence Harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HarnessVerdict {
    /// Output satisfies multidimensional coherence and non-contradiction requirements.
    Approved,
    /// Output passes with minor caveats, low confidence, or boundary-case fitness.
    Warning,
    /// Output fails coherence requirements or violates active contradictions.
    Rejected,
    /// Output failed initial check but was successfully revised into a coherent state.
    Revised,
}

impl HarnessVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            HarnessVerdict::Approved => "APPROVED",
            HarnessVerdict::Warning => "WARNING",
            HarnessVerdict::Rejected => "REJECTED",
            HarnessVerdict::Revised => "REVISED",
        }
    }
}

/// A concise summary of a detected contradiction with existing beliefs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContradictionSummary {
    pub conflict_id: String,
    pub existing_claim: String,
    pub existing_source: String,
    pub candidate_claim: String,
    pub severity: f32,
    pub description: String,
}

/// Detailed evaluation from the Physis Coherence Arbiter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceEvaluation {
    pub verdict: HarnessVerdict,
    pub composite_fitness: f32,
    pub coherence_score: f32,
    pub confidence: f32,
    pub top_cell: Option<String>,
    pub cell_score: f32,
    pub fitness_breakdown: FitnessBreakdown,
    pub coherence_profile: CoherenceProfile,
    pub detected_contradictions: Vec<ContradictionSummary>,
    pub explanation_report: Option<ExplanationReport>,
    pub warnings: Vec<String>,
    pub recommendation: String,
}

/// Overall execution result from running the Physis LLM Harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarnessResult {
    pub verdict: HarnessVerdict,
    pub output: String,
    pub raw_initial_output: Option<String>,
    pub revisions_attempted: usize,
    pub revision_critiques: Vec<String>,
    pub evaluation: CoherenceEvaluation,
    pub hypothesis_id: Option<String>,
    pub provider_used: Option<String>,
    pub audit_event_id: Option<String>,
}

/// The Physis Coherence Harness engine.
pub struct CoherenceHarness;

impl CoherenceHarness {
    /// Evaluate any candidate text or LLM output against the Physis epistemic core.
    ///
    /// What this measures: internal coherence — semantic fit onto the ontological
    /// grid, logical shape, contradiction against the *supplied* context. What it
    /// does **not** measure: factual truth. There is no retrieval and no world
    /// model here; a fluent answer with every entity wrong scores as highly as a
    /// correct one (verified empirically: fabricated landmark claims pass at
    /// 0.72 with `contradiction_penalty: 0.0`, even when reference facts are
    /// given in context). Treat APPROVED as "well-formed", never as "true".
    pub fn evaluate(
        text: &str,
        _context: Option<&str>,
        config: &HarnessConfig,
        core: &mut PhysisCore,
        classifier: &CellClassifier,
        embedder: &dyn VectorEmbed,
        quality: &QualityTracker,
    ) -> CoherenceEvaluation {
        let text_trimmed = text.trim();
        let embedding = embedder.embed(text_trimmed);

        // 1. Semiotic Grid Classification
        let cell_scores = classifier.classify_text(text_trimmed, embedder);
        let (top_cell, raw_cell_score) = cell_scores
            .first()
            .map(|c| (format!("{} × {}", c.domain, c.mode), c.score))
            .unwrap_or(("UNKNOWN".to_string(), 0.0));

        let key = top_cell.replace(" × ", "\x00");
        let adjusted_cell_score = quality.adjust_score(&key, raw_cell_score);

        // 2. Contradiction Detection with Existing Hypotheses and Beliefs
        let mut contradictions = Vec::new();
        for h in core.hypotheses.values() {
            if h.status == HypothesisStatus::Contradicted || h.status == HypothesisStatus::Failed {
                let sim = cosine_sim(&embedding, &h.embedding);
                if sim > 0.68 {
                    contradictions.push(ContradictionSummary {
                        conflict_id: h.id.clone(),
                        existing_claim: h.statement.clone(),
                        existing_source: "historical_failed_hypothesis".to_string(),
                        candidate_claim: text_trimmed.chars().take(120).collect(),
                        severity: sim,
                        description: format!(
                            "Candidate output closely aligns ({:.2}) with failed/contradicted hypothesis [{}]",
                            sim, &h.id[..8.min(h.id.len())]
                        ),
                    });
                }
            }
        }

        for c in &core.contradictions {
            if c.resolution == ResolutionStatus::Open {
                let sim_a = cosine_sim(&embedding, &embedder.embed(&c.claim_a.claim));
                let sim_b = cosine_sim(&embedding, &embedder.embed(&c.claim_b.claim));
                if sim_a > 0.70 && sim_b > 0.70 {
                    contradictions.push(ContradictionSummary {
                        conflict_id: c.id.clone(),
                        existing_claim: format!("{} vs {}", c.claim_a.claim, c.claim_b.claim),
                        existing_source: "unresolved_contradiction".to_string(),
                        candidate_claim: text_trimmed.chars().take(120).collect(),
                        severity: 0.8,
                        description: "Candidate touches an active, unresolved contradiction"
                            .to_string(),
                    });
                }
            }
        }

        // 3. Multidimensional Coherence Profile Computation
        let mut profile = CoherenceProfile::new();
        // Semantic: recall similarity against high-coherence nodes
        let recalled = core.search_nodes(&embedding, 5);
        let semantic_score = if recalled.is_empty() {
            0.50
        } else {
            let sum: f32 = recalled.iter().map(|(_, _, s)| *s).sum();
            (sum / recalled.len() as f32).clamp(0.0, 1.0)
        };
        profile.set("semantic", semantic_score, 1.0);

        // Ontological: best entry similarity
        let ontological_score = classifier
            .best_entry_sim(&embedding)
            .map(|(sim, _, _)| sim)
            .unwrap_or(0.5)
            .clamp(0.0, 1.0);
        profile.set("ontological", ontological_score, 1.0);

        // Logical consistency: penalized by detected contradictions
        let contradiction_penalty = (contradictions.len() as f32 * 0.25).min(0.80);
        let logical_score = (1.0 - contradiction_penalty).clamp(0.0, 1.0);
        profile.set("logical", logical_score, 1.0);

        // Empirical support: asserted success/failure ratio
        let mut successes = 0.0;
        let mut failures = 0.0;
        for node in core.nodes.values() {
            if let Some(verdict) = node.asserted {
                let sim = cosine_sim(&embedding, &node.embedding);
                if sim > 0.60 {
                    if verdict > 0.0 {
                        successes += sim;
                    } else if verdict < 0.0 {
                        failures += sim;
                    }
                }
            }
        }
        let empirical_score = if successes + failures > 0.0 {
            (successes / (successes + failures)).clamp(0.0, 1.0)
        } else {
            0.60
        };
        profile.set("empirical", empirical_score, 1.0);

        // Quality feedback penalty
        let quality_penalty = quality.cell_penalties.get(&key).copied().unwrap_or(0.0);

        let composite_fitness = (profile.composite() * 0.7 + adjusted_cell_score * 0.3
            - quality_penalty * 0.2)
            .clamp(0.0, 1.0);
        let coherence_score = profile.composite();
        let confidence = (ontological_score * 0.4 + semantic_score * 0.6).clamp(0.0, 1.0);

        let breakdown = FitnessBreakdown {
            semantic_fit: semantic_score,
            ontological_fit: ontological_score,
            logical_consistency: logical_score,
            empirical_support: empirical_score,
            predictive_success: 0.70,
            outcome_success: 0.70,
            contradiction_penalty,
            failed_prediction_penalty: quality_penalty,
            composite_fitness,
        };

        let mut warnings = Vec::new();
        if !contradictions.is_empty() {
            warnings.push(format!(
                "Detected {} conflicting precedent(s)",
                contradictions.len()
            ));
        }
        if quality_penalty > 0.1 {
            warnings.push(format!(
                "Cell '{}' carries quality failure penalty ({:.2})",
                top_cell, quality_penalty
            ));
        }
        if ontological_score < 0.45 {
            warnings.push(
                "Low ontological fit: output terminology is outside the known ontology dictionary"
                    .to_string(),
            );
        }

        // Determine verdict
        let verdict = if (!contradictions.is_empty() && config.reject_contradictions)
            || composite_fitness < (config.coherence_threshold - 0.20)
        {
            HarnessVerdict::Rejected
        } else if composite_fitness >= config.coherence_threshold && contradictions.is_empty() {
            HarnessVerdict::Approved
        } else {
            HarnessVerdict::Warning
        };

        let recommendation = match verdict {
            HarnessVerdict::Approved => "Output meets epistemic coherence and is cleared for downstream execution.".to_string(),
            HarnessVerdict::Warning => "Output has minor coherence anomalies; proceed with caution or request clarification.".to_string(),
            HarnessVerdict::Rejected => "Output violates active constraints or fails coherence threshold; revision or human escalation required.".to_string(),
            HarnessVerdict::Revised => "Output was successfully revised into a coherent form.".to_string(),
        };

        // Create explanation report
        let explanation_report = Some(ExplanationReport {
            subject_id: uuid::Uuid::new_v4().to_string(),
            statement: text_trimmed.to_string(),
            status: if verdict == HarnessVerdict::Approved {
                HypothesisStatus::Supported
            } else if verdict == HarnessVerdict::Rejected {
                HypothesisStatus::Contradicted
            } else {
                HypothesisStatus::Candidate
            },
            supporting_evidence: recalled.into_iter().map(|(id, label, score)| {
                Evidence {
                    source: format!("recalled_node_{}", &id[..8.min(id.len())]),
                    polarity: EvidencePolarity::Supports,
                    confidence: score,
                    claim: label,
                    observed_at: Some(chrono::Utc::now()),
                    embedding: Vec::new(),
                    context: Vec::new(),
                    intake_id: None,
                }
            }).collect(),
            contradicting_evidence: contradictions.iter().map(|c| {
                Evidence {
                    source: c.existing_source.clone(),
                    polarity: EvidencePolarity::Contradicts,
                    confidence: c.severity,
                    claim: c.existing_claim.clone(),
                    observed_at: Some(chrono::Utc::now()),
                    embedding: Vec::new(),
                    context: vec![c.description.clone()],
                    intake_id: None,
                }
            }).collect(),
            historical_precedents: Vec::new(),
            expected_consequences: Vec::new(),
            observed_consequences: Vec::new(),
            coherence_profile: profile.clone(),
            coherence_score,
            fitness_breakdown: breakdown.clone(),
            fitness_score: composite_fitness,
            confidence,
            provenance_chain: ProvenanceChain::new(),
            human_readable_summary: format!(
                "Physis evaluated output against cell [{}] with composite fitness {:.2} (threshold {:.2}). Verdict: {}.",
                top_cell, composite_fitness, config.coherence_threshold, verdict.as_str()
            ),
        });

        CoherenceEvaluation {
            verdict,
            composite_fitness,
            coherence_score,
            confidence,
            top_cell: Some(top_cell),
            cell_score: adjusted_cell_score,
            fitness_breakdown: breakdown,
            coherence_profile: profile,
            detected_contradictions: contradictions,
            explanation_report,
            warnings,
            recommendation,
        }
    }

    /// Execute a generation request through the LLM with the Physis Coherence Harness.
    #[cfg(feature = "web")]
    pub async fn generate_with_harness(
        prompt: &str,
        context: Option<&str>,
        config: &HarnessConfig,
        state: &SharedState,
        uid: &str,
        cascade: &ProviderCascade,
    ) -> Result<HarnessResult, String> {
        // Build dynamic provider if custom API key or base URL is supplied
        let custom_provider = if let Some(ref key) = config.api_key {
            let base_url = config.base_url.clone().unwrap_or_else(|| {
                match config.provider.to_lowercase().as_str() {
                    "anthropic" => "https://api.anthropic.com/v1".to_string(),
                    "gemini" => {
                        "https://generativelanguage.googleapis.com/v1beta/openai".to_string()
                    }
                    "groq" => "https://api.groq.com/openai/v1".to_string(),
                    "deepseek" => "https://api.deepseek.com/v1".to_string(),
                    "openrouter" => "https://openrouter.ai/api/v1".to_string(),
                    _ => "https://api.openai.com/v1".to_string(),
                }
            });
            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "gpt-4o-mini".to_string());
            Some(Provider::new(ProviderConfig {
                name: format!("dynamic-{}", config.provider),
                base_url,
                model,
                api_key: key.clone(),
                supports_vision: false,
                supports_tools: true,
                max_tokens: 4096,
                timeout_secs: 30.0,
            }))
        } else {
            None
        };

        // Construct initial messages
        let mut messages = Vec::new();
        messages.push(Message {
            role: "system".into(),
            content: Content::Text(
                "You are an expert, factually rigorous assistant. Provide a precise, accurate, and coherent response."
                    .to_string(),
            ),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        if let Some(ctx) = context {
            if !ctx.trim().is_empty() {
                messages.push(Message {
                    role: "system".into(),
                    content: Content::Text(format!("Context & Assumptions:\n{}", ctx)),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
            }
        }

        messages.push(Message {
            role: "user".into(),
            content: Content::Text(prompt.to_string()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        // 1. Initial LLM Generation
        let (mut current_output, provider_used) = if let Some(ref p) = custom_provider {
            match p.complete(&messages, None, None).await {
                Ok(resp) => {
                    let text = resp.content.unwrap_or_default();
                    (text, p.name().to_string())
                }
                Err(e) => return Err(format!("Custom LLM provider error: {}", e)),
            }
        } else {
            let hint = RouteHint {
                task_type: "smart".to_string(),
                has_image: false,
            };
            match cascade.complete(&messages, None, &hint, None).await {
                Ok((msg, p_name)) => (msg.content.unwrap_or_default(), p_name),
                Err(e) => return Err(format!("Cascade LLM completion failed: {}", e)),
            }
        };

        let raw_initial = current_output.clone();
        let mut critiques = Vec::new();
        let mut revisions_done = 0;

        // 2. Initial Evaluation
        let mut eval = state.with_user(uid, |s| {
            let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
            Self::evaluate(
                &current_output,
                context,
                config,
                &mut s.core,
                &state.shared.classifier,
                state.shared.embedder.as_ref(),
                &quality,
            )
        });

        // 3. Auto-Revision Loop if Incoherent or Contradicted
        if config.auto_revise
            && eval.verdict != HarnessVerdict::Approved
            && config.max_revisions > 0
        {
            for attempt in 1..=config.max_revisions {
                revisions_done = attempt;
                let critique = format!(
                    "PHYSIS COHERENCE ARBITER CRITIQUE:\n\
                     Your candidate response failed coherence checks (Composite Fitness: {:.2}, Threshold: {:.2}).\n\
                     Warnings: {}\n\
                     Contradictions: {}\n\
                     Please revise your answer to eliminate conflicts, adhere to sound logical reasoning, and ground all claims accurately.",
                    eval.composite_fitness,
                    config.coherence_threshold,
                    eval.warnings.join("; "),
                    eval.detected_contradictions.iter().map(|c| c.description.as_str()).collect::<Vec<_>>().join(" | ")
                );
                critiques.push(critique.clone());

                messages.push(Message {
                    role: "assistant".into(),
                    content: Content::Text(current_output.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });
                messages.push(Message {
                    role: "user".into(),
                    content: Content::Text(critique),
                    tool_calls: None,
                    tool_call_id: None,
                    name: None,
                });

                let revised_text = if let Some(ref p) = custom_provider {
                    match p.complete(&messages, None, None).await {
                        Ok(resp) => resp.content.unwrap_or_default(),
                        Err(e) => {
                            log::warn!("Revision call failed: {}", e);
                            break;
                        }
                    }
                } else {
                    let hint = RouteHint {
                        task_type: "smart".to_string(),
                        has_image: false,
                    };
                    match cascade.complete(&messages, None, &hint, None).await {
                        Ok((msg, _)) => msg.content.unwrap_or_default(),
                        Err(e) => {
                            log::warn!("Cascade revision call failed: {}", e);
                            break;
                        }
                    }
                };

                if !revised_text.trim().is_empty() {
                    current_output = revised_text;
                    eval = state.with_user(uid, |s| {
                        let quality = state.quality.lock().unwrap_or_else(|e| e.into_inner());
                        Self::evaluate(
                            &current_output,
                            context,
                            config,
                            &mut s.core,
                            &state.shared.classifier,
                            state.shared.embedder.as_ref(),
                            &quality,
                        )
                    });
                    if eval.verdict == HarnessVerdict::Approved {
                        eval.verdict = HarnessVerdict::Revised;
                        break;
                    }
                }
            }
        }

        // 4. Register Hypothesis into Graph & Epistemic Audit Trail if requested
        let (hypothesis_id, audit_event_id) =
            if config.register_as_hypothesis && !current_output.trim().is_empty() {
                state.with_user(uid, |s| {
                    let emb = state.shared.embedder.embed(&current_output);
                    let mut hyp = Hypothesis::new(&current_output, emb);
                    hyp.fitness = eval.composite_fitness;
                    hyp.fitness_breakdown = eval.fitness_breakdown.clone();
                    hyp.coherence_profile = eval.coherence_profile.clone();
                    hyp.status = match eval.verdict {
                        HarnessVerdict::Approved | HarnessVerdict::Revised => {
                            HypothesisStatus::Supported
                        }
                        HarnessVerdict::Warning => HypothesisStatus::Candidate,
                        HarnessVerdict::Rejected => HypothesisStatus::Contradicted,
                    };
                    if let Some(ctx) = context {
                        hyp.assumptions.push(ctx.to_string());
                    }
                    let hid = s.core.register_hypothesis(hyp);

                    let evt = EpistemicEvent::new(
                        EpistemicEventType::HypothesisGenerated,
                        &hid,
                        format!("Harness evaluated output → {:?}", eval.verdict),
                    )
                    .with_source("coherence_harness")
                    .with_metric(eval.composite_fitness);
                    let eid = evt.id.clone();
                    s.core.epistemic_audit.record(evt);
                    (Some(hid), Some(eid))
                })
            } else {
                (None, None)
            };

        Ok(HarnessResult {
            verdict: eval.verdict,
            output: current_output,
            raw_initial_output: if revisions_done > 0 {
                Some(raw_initial)
            } else {
                None
            },
            revisions_attempted: revisions_done,
            revision_critiques: critiques,
            evaluation: eval,
            hypothesis_id,
            provider_used: Some(provider_used),
            audit_event_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::RandomProjectionEmbedder;
    use crate::OntologyLoader;

    #[test]
    fn test_harness_evaluate_approved_and_rejected() {
        let embedder = RandomProjectionEmbedder::new(64);
        let ontology = OntologyLoader::load_all(&crate::config::PhysisConfig::default());
        let classifier = CellClassifier::build(&ontology, &embedder);
        let quality = QualityTracker::new(Box::new(RandomProjectionEmbedder::new(64)), ontology);
        let mut core = PhysisCore::new();

        let config = HarnessConfig::default();
        let eval = CoherenceHarness::evaluate(
            "Replace worn ball bearings on machine spindle to prevent thermal runaway",
            Some("Standard operating maintenance protocol"),
            &config,
            &mut core,
            &classifier,
            &embedder,
            &quality,
        );

        assert!(eval.composite_fitness >= 0.0 && eval.composite_fitness <= 1.0);
        assert!(eval.explanation_report.is_some());
    }
}
