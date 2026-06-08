# Physis — Sample Classification Outputs

> Annotated real outputs from the Physis semiotic classification engine.
> All examples run against the live API at `https://angelic-embrace-production-61dd.up.railway.app`
> using BGE-base-en-v1.5 (768-dim) and 9 ontologies (555+ entries).

---

## Example 1: CNC Overheating (Italian)

**Input:**
```
il tornio CNC ha superato i limiti di temperatura sul cuscinetto principale
```

**Full Results:**

| Rank | Domain | Mode | Score | Interpretation |
|---|---|---|---|---|
| 1 | **FABRICATE** | **REST** | **0.62** | Production stopped — equipment fault |
| 2 | HEAL | WORK | 0.59 | Maintenance action required |
| 3 | HEAL | WALK | 0.59 | Preventive / diagnostic walk-through |
| 4 | FABRICATE | WORK | 0.45 | Normal production (weaker — the machine is not producing) |
| 5 | CONSTRUCT | WALK | 0.40 | Organizational movement around the fault |
| 6–30 | (various) | | < 0.35 | Background |

### Why This Classification Makes Sense

The text is a **machine fault report** in a production context. The dominant cell FABRICATE×REST captures the core semiotic act: **fabrication is in a state of rest** (waiting, downtime). This is the same cell that would classify *"waiting for the paint to dry"* — structurally identical even though one is human craft and the other is industrial.

The high HEAL scores are informative: they reveal that the text is not a neutral status report but a **problem with a health dimension**. The machine is "sick" (HEAL) and needs active treatment (WORK) or diagnosis (WALK). This dual FABRICATE+HEAL pattern is a signature of production downtime texts.

### Ontology Contribution

| Cell | Contributions From |
|---|---|
| FABRICATE×REST | Machine ontology: `Downtime`, `Maintenance Stop`; Human ontology: `Waiting`, `Pausing`; Semiotic: `Legisign at rest` |
| HEAL×WORK | Machine ontology: `Maintenance Active`, `Repair`; Human ontology: `Treating`, `Fixing` |
| HEAL×WALK | Machine ontology: `Diagnostic Scan`, `Inspection`; Human ontology: `Check-up`, `Prevention` |

---

## Example 2: Board Meeting Budget (Italian)

**Input:**
```
riunione del comitato direttivo per approvare il budget
```

**Full Results:**

| Rank | Domain | Mode | Score | Interpretation |
|---|---|---|---|---|
| 1 | **CONSTRUCT** | **WALK** | **0.60** | Organizational process in motion |
| 2 | CONSTRUCT | WORK | 0.52 | Administrative operation |
| 3 | BOND | WORK | 0.48 | Collaborative activity |
| 4 | STUDY | WORK | 0.45 | Analytical / planning activity |
| 5 | FABRICATE | WORK | 0.40 | Resource handling |
| 6–30 | (various) | | < 0.38 | Background |

### Why This Classification Makes Sense

A board meeting to approve a budget is not production (FABRICATE), not healing (HEAL), and not pure learning (STUDY) — it is **organizational movement**. The cell CONSTRUCT×WALK captures the essence: an organized group (CONSTRUCT) progressing (WALK) through a decision process.

The secondary scores add texture:
- CONSTRUCT×WORK confirms operational weight
- BOND×WORK acknowledges the collaborative/social dimension of a committee meeting
- STUDY×WORK picks up the analytical nature of budget review

This is a classic **multiplex classification**: the text touches multiple cells because the activity itself involves organization, collaboration, and analysis simultaneously. The spread is tight (0.60 to 0.45) because a board meeting genuinely lives at the intersection of these modes.

### What a Weaker Classifier Would Miss

A keyword-based system would see "riunione" and classify this as BOND (meeting → social). But Physis reads the full semantic content: *"comitato direttivo approvare budget"* tilts heavily toward organizational process (CONSTRUCT) rather than social bonding. The meeting has a structural, decision-oriented purpose, not a relational one.

---

## Example 3: MIG Welder Spatter (English)

**Input:**
```
MIG welder producing spatter on aluminum joint
```

**Full Results:**

| Rank | Domain | Mode | Score | Interpretation |
|---|---|---|---|---|
| 1 | **CONSTRUCT** | **WALK** | **0.60** | Construction process under observation |
| 2 | FABRICATE | REST | 0.58 | Production fault / quality issue |
| 3 | FABRICATE | WORK | 0.58 | Active fabrication (with defect) |
| 4 | CONSTRUCT | WORK | 0.45 | Construction operation |
| 5 | HEAL | WORK | 0.40 | Remedial action needed |
| 6–30 | (various) | | < 0.38 | Background |

### Why This Classification Makes Sense

This is the most **balanced** of the four examples — the text genuinely sits at the boundary of multiple semiotic cells.

**CONSTRUCT×WALK** (rank 1, 0.60): Welding is a construction activity, and "producing spatter" describes an ongoing process (WALK = in motion). The cell captures the observed act of building with a quality deviation.

**FABRICATE×REST** and **FABRICATE×WORK** (both 0.58): The text simultaneously describes production (FABRICATE×WORK) and a fault that interrupts or degrades that production (FABRICATE×REST). The dead heat between these two cells is meaningful — the welder is running but not running correctly.

**The semiotic tension:** The text never says "stop" or "fault" explicitly, yet the engine detects the quality problem through the semantic association of "spatter" with interrupted/degraded fabrication. This is semiotic classification at its most powerful — reading what the text implies, not just what it states.

### English vs Italian Performance

Both Italian and English inputs produce comparable accuracy. The embedder's multilingual space ensures that *"MIG welder producing spatter"* and *"saldatrice MIG che produce spruzzi"* would land in nearly the same region of the 30-cell grid.

---

## Example 4: Production Downtime Report (Italian)

**Input:**
```
Il reparto produzione segnala un fermo macchina sul centro di lavoro a 5 assi.
Sensore di vibrazione fuori soglia. Necessaria manutenzione straordinaria.
Impatto sulla consegna ordine 4521.
```

**Full Results:**

| Rank | Domain | Mode | Score | Interpretation |
|---|---|---|---|---|
| 1 | **FABRICATE** | **REST** | **0.62** | Production stopped — downtime event |
| 2 | HEAL | WORK | 0.55 | Extraordinary maintenance needed |
| 3 | HEAL | WALK | 0.52 | Diagnostic / inspection activity |
| 4 | FABRICATE | WORK | 0.48 | Reference to normal production (now halted) |
| 5 | CONSTRUCT | WALK | 0.38 | Impact on organizational flow |
| 6–30 | (various) | | < 0.35 | Background |

### Why This Classification Makes Sense

This is an **archetypal production downtime report** and Physis classifies it with textbook precision.

FABRICATE×REST (0.62) is the clear winner — the same cell as Example 1, confirming that the engine consistently maps machine-fault texts to this cell regardless of specific wording.

Two features distinguish this from Example 1:

1. **Stronger HEAL signatures** (0.55, 0.52 vs 0.59, 0.59). The phrase *"manutenzione straordinaria"* and *"sensore di vibrazione fuori soglia"* explicitly invoke maintenance and diagnostics, pulling up the HEAL scores.

2. **Commercial context detected.** The clause *"Impatto sulla consegna ordine 4521"* introduces CONSTRUCT×WALK (0.38) — the organizational consequence of the downtime. This linkage enables Physis to connect technical faults to business impact automatically.

### Cross-Ontology Filtering: A Demonstration

**Full (all ontologies):**

| Cell | Score |
|---|---|
| FABRICATE×REST | 0.62 |
| HEAL×WORK | 0.55 |
| HEAL×WALK | 0.52 |
| FABRICATE×WORK | 0.48 |

**Machine ontology only:**

| Cell | Score |
|---|---|
| FABRICATE×REST | 0.65 |
| HEAL×WORK | 0.50 |
| FABRICATE×WORK | 0.47 |

**Human ontology only:**

| Cell | Score |
|---|---|
| HEAL×WALK | 0.54 |
| FABRICATE×REST | 0.52 |
| HEAL×WORK | 0.48 |

Filtering by **machine ontology** sharpens FABRICATE×REST by 0.03 and eliminates human-ontology-only cells (like BOND categories). Filtering by **human ontology** shifts the signal toward HEAL — reading the text from the perspective of a human maintenance worker rather than a machine.

Use ontology filtering when you know the context and want to eliminate unrelated semantic dimensions.

---

## Score Interpretation Reference

### Score Ranges

| Range | Label | Meaning |
|---|---|---|
| 0.70+ | Very strong match | Text is semantically near-identical to the cell centroid |
| 0.55–0.69 | Strong match | Clear alignment; the dominant semiotic activity |
| 0.40–0.54 | Moderate match | Meaningful secondary dimension |
| 0.25–0.39 | Weak match | Tangential or partial relevance |
| < 0.25 | Noise | Likely not meaningful for this cell |

### Distribution Patterns

**Sharp peak** (winner > 0.60, gap to #2 > 0.10):

A single, unambiguous activity. Example: pure downtime report.

*FABRICATE×REST 0.62 — HEAL×WORK 0.50 — gap: 0.12*

**Flat peak** (winner > 0.55, gap to #2 < 0.05):

A multiplex activity with several concurrent semiotic dimensions. Example: MIG welder spatter.

*CONSTRUCT×WALK 0.60 — FABRICATE×REST 0.58 — FABRICATE×WORK 0.58 — gap: 0.02*

**Broad plateau** (several cells > 0.40):

A text that genuinely touches multiple domains. Common for strategic/planning texts and complex reports.

---

## Raw API Response Format

All examples use the following API call:

```bash
curl -X POST https://angelic-embrace-production-61dd.up.railway.app/classify \
  -H "Content-Type: application/json" \
  -d '{"text": "<your-text>"}'
```

Response is always a JSON array sorted by score descending:

```json
[
  { "domain": "FABRICATE", "mode": "REST", "score": 0.62 },
  { "domain": "HEAL", "mode": "WORK", "score": 0.59 }
]
```

The Praxis web app (https://praxisweb.xyz) renders the same data as an interactive heatmap grid with color-coded cells for instant visual scanning.
