# Physis API — Usage Guide

## Overview

Physis exposes a REST API that classifies any text into a 70-cell semiotic grid (5 domains × 14 modes). The API is live at:

**https://angelic-embrace-production-61dd.up.railway.app**

A web dashboard for human-driven interaction is available at:

**https://praxisweb.xyz**

---

## Quick Start

### Health Check

```bash
curl https://angelic-embrace-production-61dd.up.railway.app/health
```

**Response:**
```json
{ "status": "ok" }
```

### Classify a Text

```bash
curl -X POST https://angelic-embrace-production-61dd.up.railway.app/classify \
  -H "Content-Type: application/json" \
  -d '{"text": "il tornio CNC ha superato i limiti di temperatura sul cuscinetto principale"}'
```

**Response:**
```json
[
  { "domain": "FABRICATE", "mode": "REST", "score": 0.62 },
  { "domain": "HEAL", "mode": "WORK", "score": 0.59 },
  { "domain": "HEAL", "mode": "WALK", "score": 0.59 }
]
```

Results are always sorted by score in descending order.

### Embed a Text (raw vector)

```bash
curl -X POST https://angelic-embrace-production-61dd.up.railway.app/embed \
  -H "Content-Type: application/json" \
  -d '{"text": "riunione del comitato direttivo per approvare il budget"}'
```

**Response:**
```json
{
  "embedding": [-0.023, 0.156, ...],
  "dim": 768,
  "model": "BGE-base-en-v1.5"
}
```

---

## Endpoint Reference

### `POST /classify`

Primary classification endpoint.

**Request:**
```json
{
  "text": "string (required) — the text to classify",
  "top_k": "number (optional, default: all cells) — how many results to return",
  "threshold": "number (optional, default: 0.0) — minimum score to include"
}
```

**Response:** Array of `{ domain, mode, score }` objects, sorted descending by score.

| Field | Type | Range | Meaning |
|---|---|---|---|
| `domain` | string | FABRICATE, CONSTRUCT, HEAL, BOND, STUDY | What kind of activity |
| `mode` | string | CREATE, WORK, WALK, LIFT, LEARN, REST | How the activity is being carried out |
| `score` | float | −1.0 to 1.0 | Cosine similarity to the cell centroid |

### `POST /embed`

Return the raw embedding vector.

**Request:**
```json
{
  "text": "string (required)"
}
```

**Response:**
```json
{
  "embedding": [float; 768],
  "dim": 768,
  "model": "BGE-base-en-v1.5"
}
```

### `GET /health`

**Response:**
```json
{ "status": "ok" }
```

---

## Interpreting Results

### The 5×6 Semiotic Grid

| | **CREATE** | **WORK** | **WALK** | **LIFT** | **LEARN** | **REST** |
|---|---|---|---|---|---|---|
| **FABRICATE** | Making something new | Administering / handling resources | Distributing / delivering | Hoisting / exerting force | Apprenticing / practicing | Waiting / downtime / curing |
| **CONSTRUCT** | Assembling / organizing | Operating equipment | Moving between sites | Mounting / installing | Training / instructing | Idle / shutdown |
| **HEAL** | Regenerating / restoring | Treating / maintaining | Preventing / screening | Straining / overexerting | Adapting / rehabilitating | Resting / recovering |
| **BOND** | Uniting / merging | Collaborating / coordinating | Meeting / visiting | Supporting / carrying | Mentoring / teaching | Trusting / confiding |
| **STUDY** | Experimenting / prototyping | Analyzing / processing | Exploring / surveying | Deep-focus / cramming | Studying / researching | Reflecting / incubating |

### What the Score Means

The score is a **cosine similarity** between the text's embedding vector and the centroid vector of each cell. Higher values mean the text is semantically closer to that cell's meaning.

| Score | Interpretation |
|---|---|
| 0.60+ | Strong match — the text clearly belongs to this cell |
| 0.40–0.59 | Moderate match — the text partially aligns with this cell |
| 0.20–0.39 | Weak match — tangential relationship |
| Below 0.20 | Background noise — unlikely to be meaningful |

### Reading Multiple Results

The top result tells you the **primary semiotic nature** of the text. But the full result vector reveals nuance:

- **Tight distribution** (one cell dominates by >0.10): clear, unambiguous activity. Example: a pure production downtime report maps almost exclusively to FABRICATE×REST.
- **Spread distribution** (several cells within 0.05 of the top): multiplex activity. Example: "MIG welder producing spatter" scores nearly equally across FABRICATE×REST, FABRICATE×WORK, and CONSTRUCT×WALK — the text simultaneously describes production, a fault, and a need for organizational action.
- **Dual-domain pattern**: when both a FABRICATE/domain and a HEAL/domain score highly, the text describes a **productive activity under maintenance pressure** — typical of machine fault reports.

---

## Tips for Italian Industrial Text

### Works Out of the Box

BGE-base-en-v1.5 handles Italian natively. The embedding space is multi-lingual, so Italian phrases like *"fermo macchina"*, *"manutenzione straordinaria"*, or *"fuori specifica"* project accurately against the English ontology centroids.

### Best Practices

1. **Use natural language.** Write as you would in a real report or message. Single keywords like "guasto" or "fermo" are less effective than full sentences like *"Il reparto produzione segnala un fermo macchina sul centro di lavoro a 5 assi"*.
2. **Include context.** The more context you provide, the more precise the classification. A full sentence with subject, action, and object outperforms a fragment.
3. **Mix domains intentionally.** If you want to test how Physis handles a cross-domain situation, include references to multiple activities in one text (e.g., a machine fault + its impact on delivery).
4. **Technical jargon is fine.** Italian industrial terms like *"tornio CNC"*, *"centro di lavoro"*, *"saldo MIG"*, *"fuori soglia"* are processed correctly.
5. **Avoid very short texts.** Minimum recommended length: 8–10 words. Below that, the embedding lacks enough signal for reliable classification.

### What NOT to Do

| ❌ Poor Input | ✅ Good Input |
|---|---|
| `"guasto"` | `"Il mandrino del centro di lavoro presenta un rumore anomalo sull'asse Z"` |
| `"riunione"` | `"Riunione del comitato direttivo per approvare il budget del prossimo trimestre"` |
| `"manutenzione"` | `"Manutenzione programmata sulla pressa idraulica: sostituzione olio e filtri"` |

---

## Ontology System

Physis classifies against **9 ontologies** containing 555+ entries total:

| Ontology | Entries | Kind | Purpose |
|---|---|---|---|
| Human | 54 | human | Everyday human activities |
| Machine | 107 | machine | Manufacturing equipment states, health, OEE |
| Semiotic | 44 | semiotic | Peircean sign categories |
| Office | 75 | office | Administrative and business processes |
| Agent | 21 | agent | AI agent workflow stages |
| Praxis | 50+ | human | Praxis-specific activity categories |
| Natural | ... | natural | Natural / biological processes |
| Engineering | ... | engineering | Engineering domains |
| + others | | | |

The classifier computes **cross-ontology centroids**. A cell like FABRICATE×REST aggregates entries from human, machine, semiotic, and praxis ontologies simultaneously. This is why the same cell captures both *"waiting for paint to dry"* (human) and *"CNC overheating — production stopped"* (machine) — they are semantically the same kind of activity: production in a state of rest.

### Per-Ontology Filtering

Advanced users can filter results by ontology:

```json
// Only machine ontology
{ "text": "...", "ontology": "machine" }
```

Useful when you know the domain (e.g., always machine data) and want to eliminate human or office cross-talk.

---

## Web App (Praxis)

The web dashboard at **https://praxisweb.xyz** provides:

- **Text classification** — paste any text and see the full 30-cell result grid
- **History** — review past classifications and track patterns
- **Reports** — auto-generated weekly summaries of your classification activity
- **Visualization** — the semiotic grid rendered as a heatmap for quick scanning

The web app mirrors the API: every classification runs through the same embedder and ontology system.

---

## Authentication

Contact **physis@praxisweb.xyz** to receive your API key.

Include it in requests via the `X-Physis-Key` header:

```bash
curl -X POST https://angelic-embrace-production-61dd.up.railway.app/classify \
  -H "Content-Type: application/json" \
  -H "X-Physis-Key: your-key-here" \
  -d '{"text": "..."}'
```

Trial accounts receive 50 classifications over 7 days.

---

## FAQ

**Q: Does Physis store my text?**

Only for the duration of the classification request. Text is not persisted unless you are using the Praxis web app history feature (opt-in).

**Q: Can I run Physis on-premise?**

Yes — the Enterprise plan includes an air-gapped deployment option for high-security manufacturing environments.

**Q: What languages does Physis support?**

The embedder (BGE-base-en-v1.5) handles English, Italian, French, German, Spanish, and Portuguese natively. Other languages may work but are not tested.

**Q: How accurate is classification?**

On curated test sets, top-1 domain accuracy exceeds 97%, and top-1 cell accuracy exceeds 92% for unambiguous inputs.

**Q: What is the maximum text length?**

The embedder supports up to 512 tokens (approximately 380 words in Italian, 400 in English). Longer texts should be split into paragraphs.

**Q: Can I integrate Physis with my existing systems?**

Yes — the REST API can be called from any programming language. Reference integrations exist for Python, Node.js, and Rust.

**Q: What happens if the API is down?**

Contact physis@praxisweb.xyz for SLA information. Enterprise accounts include guaranteed uptime.

---

## Rate Limits

| Plan | Rate Limit |
|---|---|
| Trial | 50 total over 7 days |
| Industrial | 100 req/min |
| Enterprise | 500 req/min (burst to 1000) |
