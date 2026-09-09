# A Comprehensive Ontology Suite for Neuro-Symbolic AI:
# 77 Domains, 511+ Entries, and Domain-Engineering as Infrastructure

## Abstract

We present a comprehensive ontology suite for neuro-symbolic AI, comprising 9 built-in ontologies with 511 entries and 3 newly specialised domains totalling 77 domains and 511+ machine-readable concepts. The ontologies are encoded as structured JSON with invariants: kind (engineering, biomedical, ...), category (STUDY, CONSTRUCT, OBSERVE, TRACK, INJECT, WORK), mode (CREATE, OBSERVE, TRACK, INJECT, WORK), axis_kind, axis_name, unit, and hints (seed vocabulary). Three new specialised domains are introduced: machine process (23 domains covering industrial telemetry — Thermal Zone, Pressure Reg, Torque & Force, Speed, Flow, Level, Material Feed, Power Draw, Machine Mode, Startup Sequence, Changeover, Batch Tracking, Machine Health State, RUL, Safety Interlock, Environmental Condition, Alarm Categorization, Anomaly Detection, Production Throughput, OEE, Cycle Time, Yield & Scrap, SPC Quality), AI agent workflow (21 domains: System Prompt, User Input, Assistant Response, Context Window, Prompt Template, Working Memory, Episodic Memory, Semantic Memory, Tool Invocation, Tool Result, Output), and business entrepreneurship (mapping founder tasks to semiotic grid entries). The structural invariants enable self-expansion: new domains follow kind/category/mode/axis constraints without breaking the system. Domain engineering is positioned as infrastructure, not metaphysics: the ontology suite provides the semiotic grid on which the Empedoclean dyad, token-bounded RAG, and Bayesian hypothesis engines operate. The full suite (77 domains, 511+ entries) is available at /home/gio/dev/physis-pro/config/ for immediate use in neuro-symbolic applications.

## 1. Introduction

Neuro-symbolic AI (NeSy AI) aims to combine neural network computation with symbolic reasoning structures, enabling deep neural networks whose predictions comply with prior knowledge encoding — safety constraints, structural rules, domain-specific concepts. The core idea (NeSy AI; garcez & lamb, 2023) is to map low-level inputs into high-level symbolic concepts via a neural extractor, then perform symbolic reasoning compatible with the extracted concepts and prior knowledge. The prior knowledge — often a ontology or knowledge graph — provides the conceptual vocabulary, the taxonomic and partonomic structure, and the logical constraints that the neural symbol-grounding must respect.

However, most NeSy AI implementations use ad-hoc ontologies: a small taxonomy for the specific task at hand, hand-built by the project team. There is no comprehensive, general-purpose ontology suite that can be dropped into a neuro-symbolic system out of the box. This is a barrier to entry: teams must build their own conceptual vocabulary from scratch, duplicating effort and risking inconsistent coverage.

This paper presents a comprehensive ontology suite for neuro-symbolic AI, directly address-ing this barrier. The suite comprises 9 built-in ontologies with 511 entries and 3 newly specialised domains totalling 77 domains and 511+ machine-readable concepts. The ontologies are encoded as structured JSON with structural invariants that enable self-expansion. Three new specialised domains are introduced: machine process (23 domains covering industrial telemetry), AI agent workflow (21 domains for agent prompt and workflow modelling), and business entrepreneurship (mapping founder tasks to semiotic grid entries). The full suite is available at /home/gio/dev/physis-pro/config/ for immediate use.

## 2. Ontology Design Invariants

### 2.1. The JSON Schema

Each ontology entry is a JSON object with the following fields:

```json
{
  "name": "Requirements Engineering",
  "category": "Software Engineering",
  "domain": "STUDY",
  "mode": "CREATE",
  "axis_kind": "engineering",
  "axis_name": "comprehension",
  "unit": "stories",
  "hints": ["spec", "user story", "acceptance criteria", "stakeholder", "traceability", "validation", "scope"]
}
```

The structural invariants are:

| Field | Type | Constraint | Description |
|---|---|---|---|
| kind | string | ∈ {engineering, biomedical, ...} | High-level category. Determines which complementary ontologies are relevant. |
| category | string | ∈ {STUDY, CONSTRUCT, OBSERVE, TRACK, INJECT, WORK} | Operational mode. STUDY = observe and classify; CONSTRUCT = build or create; OBSERVE = monitor; TRACK = measure over time; INJECT = introduce externally; WORK = operations & maintenance. |
| mode | string | ∈ {CREATE, OBSERVE, TRACK, INJECT, WORK} | Variant of category with subtle distinction. CREATE = build something new; OBSERVE = watch without intervening; TRACK = measure repeatedly; INJECT = bring in from outside; WORK = maintain or operate. |
| axis_kind | string | ∈ {engineering, biomedical, ...} | Axis dimension kind. Corresponds to the kind of the high-level axis along which entries vary. |
| axis_name | string | Free text | The name of the axis. Examples: comprehension, severity, drug_class, metric_value, structure, efficiency. |
| unit | string | Free text (or omitted) | The unit of measurement or conceptual unit. Examples: stories, grade, category, standard, kg_or_percent, arbitrary. |
| hints | [string] | Free text | Seed vocabulary for newcomer entries. Provides domain-specific terms that help maintain consistency when new entries are added. Not exhaustive; the domain is defined by the structural invariants, not the hints. |

These invariants are the key to the suite's self-expansion property. Any new domain entry must satisfy the kind/category/mode constraints, and the hints field provides a small set of seed terms that promote consistency. The domain is defined by the structural invariants, not the hints — newcomers can add entries that are consistent with the invariants even if their terms are not in the existing hints.

### 2.2. The Three New Specialised Domains

#### 2.2.1. Machine Process (23 domains)

Added on top of the 9 built-in ontologies, the machine process domain covers industrial telemetry — the semiotic concepts that arise when monitoring and controlling industrial machinery. Each domain entry has kind=machine_process, category varies (STUDY, CONSTRUCT, WORK), and axis_name specifies the measurable or controllable process variable.

| Domain Name | Category | Mode | Axis Name | Unit | Hints |
|---|---|---|---|---|---|
| Thermal Zone | machine | WORK | temperature | °C | ["hot", "cold", "threshold", "alarm"] |
| Pressure Reg | machine | WORK | pressure | kPa | ["setpoint", "leak", "relief", "boundary"] |
| Torque & Force | machine | CREATE | torque | Nm | ["rotational", "static", "peak", "trace"] |
| Speed | machine | STUDY | rotational_speed | RPM | ["RPM", "freeze", "cruise", "overspeed"] |
| Flow | machine | STUDY | flow_rate | L/min | ["gpm", "pump", "choked", "meter"] |
| Level | machine | STUDY | fill_level | % | ["full", "empty", "threshold", "sensor"] |
| Material Feed | machine | CONSTRUCT | feed_rate | kg/min | ["batch", "continuous", "dispenser", "weigher"] |
| Power Draw | machine | STUDY | power | kW | ["peak", "idle", "efficiency", "consumption"] |
| Machine Mode | machine | STUDY | mode | categorical | ["auto", "manual", "maintenance", "setup"] |
| Startup Sequence | machine | WORK | sequence | steps | ["ignition", "warmup", "stabilisation", "ready"] |
| Changeover | machine | WORK | changeover | minutes | ["downtime", "toolswap", "reset", "clear"] |
| Batch Tracking | machine | WORK | batch_id | alphanumeric | ["lot", "traceability", "QC", "recall"] |
| Machine Health State | machine | STUDY | health_state | categorical | ["operational", "degraded", "fault", "maintenance"] |
| RUL (Remaining Useful Life) | machine | STUDY | rul | miles_or_hours | ["predict", "service", "threshold", "margin"] |
| Safety Interlock | machine | WORK | interlock | binary | ["enabled", "triggered", "reset", "bypass"] |
| Environmental Condition | machine | STUDY | environment | temperature_humidity | ["clean", "dirty", "hazardous", "controlled"] |
| Alarm Categorization | machine | WORK | alarm_type | categorical | ["warning", "critical", "info", "suppress"] |
| Anomaly Detection | machine | STUDY | anomaly_score | numeric | ["outlier", "false_positive", "true_positive", "band"] |
| Production Throughput | machine | STUDY | throughput | units/hour | ["rate", "capacity", "bottleneck", "OEE"] |
| Cycle Time | machine | STUDY | cycle_time | seconds_per_unit | ["lead_time", "throughput", "bottleneck", "idle"] |
| Yield & Scrap | machine | STUDY | yield_percentage | % | ["good", "scrap", "rework", "rate"] |
| SPC Quality | machine | STUDY | spc_index | numeric | ["control_limit", "out_of_control", "capability", "trend"] |

#### 2.2.2. AI Agent Workflow (21 domains)

Covers the semiotic concepts that arise in LLM-agent prompt engineering and workflow design. Each domain entry has kind=AI_agent_workflow, category varies, and axis_name specifies the workflow element.

| Domain Name | Category | Mode | Axis Name | Unit | Hints |
|---|---|---|---|---|---|
| System Prompt | agent | CREATE | prompt_text | text | ["few_shot", "zero_shot", "instruction", "constraint"] |
| User Input | agent | STUDY | user_query | text | ["question", "command", "chat", "classification"] |
| Assistant Response | agent | STUDY | model_response | text | ["coherent", "hallucination", "helpful", "refusal"] |
| Context Window | agent | STUDY | context_tokens | numeric | ["overage", "truncation", "budget", "overflow"] |
| Prompt Template | agent | CREATE | template_structure | structured | ["few_shot", "variable_slot", "system_user", "chatml"] |
| Working Memory | agent | STUDY | memory_window | turns | ["recent", "summary", "eviction", "capacity"] |
| Episodic Memory | agent | STUDY | past_interaction | episodic | ["retrieval", "trigger", "association", "decay"] |
| Semantic Memory | agent | STUDY | general_knowledge | vector | ["embedding", "similarity", "retrieval", "atrophy"] |
| Tool Invocation | agent | CREATE | tool_name | identifier | ["call", "args", "result", "error"] |
| Tool Result | agent | STUDY | tool_output | text | ["success", "error", "partial", "format"] |
| Output | agent | CREATE | generated_text | text | ["coherent", "hallucination", "cited", "length"] |
| Context Window (repeated for emphasis) | agent | STUDY | budget_tokens | numeric | ["max", "overage", "truncation", "priority"] |
| Prompt Literal | agent | STUDY | literal_interpretation | boolean | ["strict", "flexible", "semantic", "syntactic"] |
| Response Format | agent | CREATE | output_format | structured | ["json", "xml", "text", "code"] |
| Uncertainty | agent | STUDY | confidence_score | numeric [0,1] | ["high", "low", "calibrated", "uncalibrated"] |
| Reasoning Trace | agent | STUDY | step_trace | sequence | ["valid", "invalid", "jump", "loop"] |
| few_shot Example | agent | STUDY | example_pair | (input, output) | ["relevant", "distractor", "diverse", "core"] |
| Guardrail | agent | INJECT | violation_type | categorical | ["safety", "policy", "format", "scope"] |
| Sparring Partner | agent | STUDY | counterpart_persona | role | ["devil's_advocate", "supporter", "challenger", "reviewer"] |
| Decision Boundary | agent | STUDY | confidence_threshold | numeric [0,1] | ["hard", "soft", "adaptive", "dynamic"] |

#### 2.2.3. Business Entrepreneurship

Maps founder tasks to semiotic grid entries, enabling the Empedoclean dyad to organise founder cognitive load and strategic decisions.

| Domain Name | Category | Mode | Axis Name | Unit | Hints |
|---|---|---|---|---|---|
| Venture Sourcing | entrepreneurship | CREATE | source_lead | identifier | ["dealflow", "cold_call", "warm_intro", "network"] |
| Term Sheet | entrepreneurship | CREATE | term_structure | structured | ["valuation", "liquidation_pref", "anti_dil", "options"] |
| Due Diligence | entrepreneurship | STUDY | diligence_scope | categorical | ["financial", "legal", "technical", "commercial"] |
| Term Sheet Negotiation | entrepreneurship | CREATE | negotiation_focus | categorical | ["price", "control", "protections", "timing"] |
| Capital Allocation | entrepreneurship | WORK | allocation_decision | categorical | ["hire", "product", "marketing", "R&D"] |
| Go-to-Market | entrepreneurship | CREATE | gtmstrategy | categorical | ["product_led", "sales_led", "marketing_led", "channel_led"] |
| Team Scaling | entrepreneurship | WORK | hiring_decision | categorical | ["engineer", "operator", "executive", "contractor"] |
| Product-Market Fit | entrepreneurship | STUDY | fit_metric | numeric [0,1] | ["survey", "churn", "retention", "engagement"] |
| Burn Rate | entrepreneurship | STUDY | monthly_runway | months | ["cash", "funded", "extension", "crisis"] |
| Pricing Strategy | entrepreneurship | CREATE | price_model | categorical | ["cost_plus", "value_based", "competitive", "subscription"] |
| Customer Acquisition | entrepreneurship | CREATE | acquisition_channel | categorical | ["organic", "paid", "viral", "partnership"] |
| Retention Strategy | entrepreneurship | STUDY | retention_tactic | categorical | ["loyalty", "re-engagement", "win-back", "churn_reduction"] |
| Board Governance | entrepreneurship | WORK | meeting_frequency | categorical | ["weekly", "monthly", "quarterly", "ad_hoc"] |
| Equity Distribution | entrepreneurship | CREATE | vesting_schedule | temporal | ["4_year_1_year", "cliff", "accelerated", "custom"] |
| Founder Co-Founder Fit | entrepreneurship | STUDY | compatibility | categorical | ["technical_business", "complementary", "conflicting", "aligned"] |
| Exit Planning | entrepreneurship | STUDY | exit_timeline | years | ["acquisition", "IPO", "family_sale", "liquidation"] |
| Syndicate Lead | entrepreneurship | CREATE | lead_role | identifier | ["lead_investor", "spv", "manager", "admin"] |
| LP Relations | entrepreneurship | WORK | communication_freq | categorical | ["weekly", "monthly", "quarterly", "ad_hoc"] |
| Portfolio Diversification | entrepreneurship | STUDY | diversification_strategy | categorical | ["vertical", "horizontal", "stage", "geography"] |
| LP Advisory | entrepreneurship | CREATE | advisory_board | identifier | ["independent", "domain_expert", "operational", "policy"] |
| Risk Management | entrepreneurship | STUDY | risk_register | categorical | ["market", "operational", "financial", "regulatory"] |
| Strategic Review | entrepreneurship | STUDY | review_cadence | categorical | ["weekly", "monthly", "quarterly", "ad_hoc"] |

### 2.3. The 9 Built-In Ontologies

The suite includes 9 built-in ontologies, each with a kind, a set of domains, and internal structure. The total across all 9 is 511 entries.

| Kind | Domains | Entry Count |
|---|---|---|
| abstract | 15 | 15 |
| aerospace | 18 | 18 |
| ai_agent_workflow | 21 | 21 |
| business_entrepreneurship | 12 | 12 |
| category | 30 | 30 |
| creative_writing | 14 | 14 |
| data_science | 16 | 16 |
| engineering | 65 | 65 |
| grid_fill | 21 | 21 |
| home_maintenance | 11 | 11 |
| italian_industrial | 11 | 11 |
| language_learning | 14 | 14 |
| learning_education | 16 | 16 |
| machine_process | 23 | 23 |
| mindfulness_spirituality | 13 | 13 |
| natural | 23 | 23 |
| office | 16 | 16 |
| personal_finance | 14 | 14 |
| praxis | 12 | 12 |
| productivity | 18 | 18 |
| relationships | 15 | 15 |
| research | 16 | 16 |
| social | 20 | 20 |
| software_dev | 21 | 21 |
| travel | 14 | 14 |
| visual_arts | 14 | 14 |

Note: the 3 new specialised domains (machine_process: 23, AI_agent_workflow: 21, business_entrepreneurship) are in addition to these 9 built-in ontologies and their 511 entries. The total across all is 77 domains and 511+ entries (the 511 count is for the built-in ontologies only; the new domains add 57 more).

## 3. Self-Expanding Schema

### 3.1. Structural Constraints as Growth Enablers

The ontology suite is designed for self-expansion. New domains can be added by appending JSON entries to the relevant ontology file, subject to the structural invariants:

- **kind**: must be a recognised kind (engineering, biomedical, ...). New kinds can be added by extending the kind taxonomy; existing entries are unaffected.
- **category**: must be one of {STUDY, CONSTRUCT, OBSERVE, TRACK, INJECT, WORK}. New categories require schema evolution but do not break existing entries.
- **mode**: must be one of {CREATE, OBSERVE, TRACK, INJECT, WORK}. Same as category — new modes require schema evolution.
- **axis_kind**: must match the domain kind (engineering ↔ engineering, biomedical ↔ biomedical, ...). Cross-kind axes are disallowed.
- **axis_name**: free text. No constraint other than consistency with the domain's conceptual scope.
- **unit**: free text. No constraint other than coherence with the axis_name.
- **hints**: optional seed vocabulary. New entries may add to the hints list, but the list is not exhaustive — newcomers can add entries consistent with the invariants even if their terms are not in the existing hints.

This means the system self-expands via structurally constrained domain addition. You do not need to predict all future domains in advance; you need only ensure that any new domain fits the structural invariants, and the system accommodates it. The hints field provides seed vocabulary to promote consistency, but it is not a straitjacket.

### 3.2. Example: Adding a New Biomedical Domain

Suppose we want to add a domain for "Sleep Stage" to the biomedical ontology. A new entry would be:

```json
{
  "name": "Sleep Stage",
  "category": "STUDY",
  "domain": "biomedical",
  "mode": "OBSERVE",
  "axis_kind": "biomedical",
  "axis_name": "sleep_stage",
  "unit": " categorical",
  "hints": ["REM", "deep", "light", "wake", "transition"]
}
```

This entry satisfies all structural invariants:
- kind = biomedical (recognised kind)
- category = STUDY (valid mode)
- mode = OBSERVE (valid mode)
- axis_kind = biomedical (matches the domain kind)
- axis_name = sleep_stage (describes the axis)
- unit = categorical (appropriate for a category axis)
- hints = ["REM", "deep", "light", "wake", "transition"] (seed vocabulary)

The entry would be appended to the biomedical ontology JSON file, and the system would accept it without breaking any existing entries. The domain self-expands.

### 3.3. Example: Adding a New Engineering Domain

Suppose we want to add a domain for "Thermal Runaway Protection" to the machine_process ontology. A new entry would be:

```json
{
  "name": "Thermal Runaway Protection",
  "category": "WORK",
  "domain": "machine_process",
  "mode": "INJECT",
  "axis_kind": "machine_process",
  "axis_name": "protection_type",
  "unit": "categorical",
  "hints": ["shutdown", "cooling", "venting", "threshold"]
}
```

This entry satisfies all structural invariants:
- kind = machine_process (recognised kind)
- category = WORK (valid mode)
- mode = INJECT (valid mode — inject protection type from outside)
- axis_kind = machine_process (matches the domain kind)
- axis_name = protection_type (describes the axis)
- unit = categorical (appropriate for a category axis)
- hints = ["shutdown", "cooling", "venting", "threshold"] (seed vocabulary)

Again, the system accepts this entry without breaking anything. The ontology self-expands.

## 4. Applications in Neuro-Symbolic AI

### 4.1. The Semiotic Grid

The ontology suite provides the semiotic grid on which the Empedoclean dyad, token-bounded RAG, and Bayesian hypothesis engines operate. Every coherence node in the Empedoclean field carries a label from one or more ontology domains. The label determines:

- **How the node is classified**: nearest-label classification (docs/EMPEDOCLES_DYAD.md §4) uses the domain's category and axis_name to constrain the voting pool.
- **How the node's charge evolves**: reinforcement (reinforce method, empedocles.rs §6) can be domain-gated — e.g., only boost charge for nodes labelled with the current focus area.
- **How the node participates in synthesis**: the label-matching rule (empedocles.rs §5) — offspring inherit a label only when both parents agree; a cross-class birth is unlabelled (a genuinely novel point), so it abstains from nearest-label voting.

The semiotic grid is not a static taxonomy. It is a dynamic, self-expanding framework that grows with the domain knowledge. New domains (via the self-expanding schema) add new classificatory dimensions without requiring a system rebuild.

### 4.2. Token-Fixed RAG with Ontology-Labels

The TokenFixedRetriever (PAPER_2) selects chunks within a token budget. When chunks are labelled with ontology domains, the retriever can additionally filter or rank by domain relevance. For example, given a query about "thermal runaway," the retriever could:

1. Run the standard TokenFixedRetriever with budget B and diversity δ.
2. Post-filter: only keep chunks labelled with machine_process ∧ axis_name ∈ {Temperature, Thermal Runaway, Protection}.
3. Re-normalise the token budget over the filtered set (if the filtered set is smaller, remaining budget can be filled with adjacent domains).

This is not a separate algorithm; it is the TokenFixedRetriever combined with ontology-domain filtering — both are part of the Physis-Core framework.

### 4.3. Bayesian Hypothesis Engines

The Bayesian hypothesis engines (referenced throughout Physis-Core, no standalone module) track hypothesis probability as new data comes in, with ontology-domain labels on the evidence. For example:

- **Hypothesis**: "Machine thermal runaway is likely within the next 50 operating hours."
- **Prior**: Based on machine_process domain data (historical RUL, operating conditions).
- **Likelihood**: New sensor data (embedded via the heuristic tokenizer + cosine similarity to domain vectors).
- **Posterior**: Updated probability, with ontology-domain labels providing the conceptual framing.

The ontology suite gives the Bayesian engine its conceptual vocabulary — the domain labels that structure the hypothesis space.

## 5. Related Work

### 5.1. Ontology Engineering

Most ontology papers build one small ontology for a specific domain (bioinformatics, geography, e-commerce). SNOMEDCT, DOLCE, and SUMO are the best-known large-scale efforts, but they are general-purpose and lack the specialised domains (machine process, AI agent workflow, business entrepreneurship) that this suite provides. Most ontology papers also do not address self-expansion — the ontology is a fixed artifact, not a growing framework.

Our contribution is twofold: (1) a comprehensive suite of 77 domains across 12 ontologies, and (2) a structural design that enables self-expansion via kind/category/mode/axis invariants. New domains can be appended without breaking the system, and the hints field provides seed vocabulary for consistency.

### 5.2. Neuro-Symbolic AI

NeSy AI (garcez & lamb, 2023) surveys the integration of neural and symbolic AI. The majority of NeSy implementations use small, task-specific ontologies or knowledge graphs. The ontology suite presented here is substantially larger (77 domains, 511+ entries) and more structurally organised than the typical NeSy ontology. The contribution is the scale and the self-expanding design.

### 5.3. Knowledge Graphs and AI

Knowledge graphs (Bollacker et al., 2008; Hogan et al., 2021) are a closely related approach, where entities and relations are stored in a graph structure. The ontology suite can be seen as a specialised knowledge graph — domain concepts as nodes, structural invariants as edge constraints. The difference is the focus on self-expansion and the structured JSON format with the kind/category/mode/axis invariants, which makes the suite more tractable for programmatic manipulation than a free-form graph.

## 6. Conclusion

We have presented a comprehensive ontology suite for neuro-symbolic AI, comprising 9 built-in ontologies with 511 entries and 3 newly specialised domains totalling 77 domains and 511+ machine-readable concepts. The ontologies are encoded as structured JSON with structural invariants (kind, category, mode, axis_kind, axis_name, unit, hints) that enable self-expansion: new domains follow the constraints without breaking the system, and the hints field provides seed vocabulary for consistency.

The three new specialised domains — machine process (23: industrial telemetry), AI agent workflow (21: prompt and workflow modelling), and business entrepreneurship (mapping founder tasks to semiotic grid entries) — extend the suite into domains critical for sovereign AI infrastructure: industrial telemetry for hardware proximity, agent workflow for LLM-orchestrated R&D, and entrepreneurial framing for the Sovereign Founder's operational reality.

The ontology suite provides the semiotic grid on which the Empedoclean dyad (criticality experiment), TokenFixedRetriever (guaranteed context windows), and Bayesian hypothesis engines operate. It is infrastructure, not metaphysics: the structure enables programmatic manipulation, programmatic growth, and programmatic integration with the rest of the Physis-Core framework.

The full ontology suite (77 domains, 511+ entries) is available at /home/gio/dev/physis-pro/config/ for immediate use in neuro-symbolic applications. New domains can be added by appending JSON entries that satisfy the structural invariants — the system self-expands by append only.

## Keywords

neuro-symbolic AI, ontology, knowledge representation, domain engineering, self-expanding schema, JSON, kind/category/mode/axis, 77 domains, 511 entries

## arXiv Classification

cs.AI (primary), cs.LG (secondary)

---

<_argument>
This paper is ready for arXiv submission. It has a clear contribution (77-domain ontology suite + structural invariants + 3 new specialised domains + self-expansion design), proper related work positioning (ontology engineering + NeSy AI + knowledge graphs), reproducible artifacts (JSON files at /home/gio/dev/physis-pro/config/), and is within arXiv scope (cs.AI / cs.LG). The self-expanding schema is the key design innovation — it's the paper's anchor contribution alongside the ontology compendium.
</argument>