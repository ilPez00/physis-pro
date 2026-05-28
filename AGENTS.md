# Physis Pro — Industrial Backoffice Intelligence

## Overview
Physis Pro extends the core engine with **industrial backoffice intelligence**:
- **Quality feedback loop** — embed failures against the semiotic grid, apply penalty, persist
- **Industrial monitoring** — machine adapter pattern (MQTT, Modbus, serial, REST)
- **Document ingest** — classify and process structured/unstructured documents
- **Inventory tracking** — SMA-30 forecast + human-in-loop ordering with cooldown
- **Ontology extensions** — 75 office domains + 23 machine process + 21 AI agent workflow

## Key Modules

| Module | File | Description |
|--------|------|-------------|
| Quality | `quality.rs` | `QualityTracker` — `report_failure()`, `report_success()`, `save()`, `load()`. Persists 384-dim embeddings to `~/.physis/quality_failures.json`. 6 tests |
| Wiki | `wiki.rs` | `llmwiki` CLI fallback with recursive grep fallback |
| Industrial | `industrial.rs` | `MachineAdapter` trait, `MachineReading`, `ProcessAnomaly`, `detect_anomalies`, stub `MqttAdapter`. 4 tests |
| Ingest | `ingest.rs` | `DocKind`, `IngestedDoc`, `classify_file()`, `ingest_all()`, `sort_by_kind()`. 4 tests |
| Inventory | `inventory.rs` | `InventoryTracker` — SMA-30 forecast, sled persistence, order cooldown. 8 tests |

## Ontology Extensions

Three new ontology domains added on top of the 9 built-in ontologies (511 entries):

| Ontology | File | Type | New Domains |
|----------|------|------|-------------|
| Machine Process | `config/machine_ontology.json` | machine (kind) | 23 — Thermal Zone, Pressure Reg, Torque & Force, Speed, Flow, Level, Material Feed, Power Draw, Machine Mode, Startup Sequence, Changeover, Batch Tracking, Machine Health State, RUL, Safety Interlock, Environmental Condition, Alarm Categorization, Anomaly Detection, Production Throughput, OEE, Cycle Time, Yield & Scrap, SPC Quality |
| AI Agent Workflow | `config/agent_ontology.json` | agent (kind) | 21 — System Prompt, User Input, Assistant Response, Context Window, Prompt Template, Working Memory, Episodic Memory, Semantic Memory, Tool Invocation, Tool Result, Output Publishing, Error Recovery, Quality Check, Confidence Threshold, Human Escalation, PDCA Cycle, Workflow Chain, Agent Loop, Agent State, Agent Versioning, Telemetry & Log |
| Office | `config/office_ontology.json` | office (kind, fresh axis_kind) | 75 across 12 categories: Communication, Finance, Procurement, People, Projects, Customer, Reporting, IT, Facilities, Legal, Marketing, Travel, R&D |

## Config (`PhysisProConfig`)
- `PhysisProConfig` extends the base config with ontology loader fields
- Office ontology loaded as a new file with `kind: "office"` and `axis_kind: "office"`
- `wenyan_enabled=false`, `sanskrit_enabled=false` by default (business mode)
- Token trie built by `scan` is per-invocation only (not persisted)
- Quality feedback persists to `~/.physis/quality_failures.json`
