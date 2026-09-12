# What Physis is

**Read this before summarising Physis to anyone, including yourself.**

This file exists because three separate attempts in one session compressed a
190-module, ~113k-line system into a single noun, and all three were wrong:

| attempt | claim | why it was wrong |
|---|---|---|
| 1 | "Physis is a context compiler." | Context compiling is one operation out of six. MIT alternatives beat it on token reduction. |
| 2 | "No — an epistemic ledger." | The ledger is a second operation out of six. |
| 3 | "70-symbol vocabulary, one n-gram table." | It is **70 tables with subtables**. See §2. |

Each attempt sounded sharper than the last and was equally wrong. **The system
does not compress to a noun. Describe it structurally.**

---

## 1. The one true sentence

> Physis is a **coordinate system for meaning** — 5 domains × 14 modes = 70
> cells, tree-nested by facets — **plus the operations that assign, revise and
> audit coordinates**, plus the applications built on those operations.

Everything else is a consequence of that sentence.

---

## 2. The coordinate system

```rust
DOMAINS: [HEAL, CONSTRUCT, FABRICATE, BOND, STUDY]                       //  5
MODES:   [LIFT, REST, WALK, WORK, CREATE, LEARN, DESTROY, SENSE,
          GUIDE, PLAY, BRAINSTORM, MAINTAIN, MOVE, PLAN]                 // 14
```
`src/semiotic_grid.rs` — **5 × 14 = 70 cells.** Stable public API; ontology packs
add entries, never axes.

Facets tree-nest *under* a cell **without enlarging the base grid**
(`physis-core/src/models.rs`, `struct Facets`):

| facet | type |
|---|---|
| `sub_domain` | free string |
| `sub_mode` | free string |
| `lifecycle` | `LifecyclePhase` |
| `agency` | `Agency` |
| `scale` | `Scale` |
| `abstraction` | `Abstraction` |

**So the unit is not "70 symbols". It is 70 tables, each with subtables.** A flat
70-symbol vocabulary models `P(next cell | previous cells)` and throws the facets
away. The architecture is a *conditional model per region*:
`P(next | cell, sub_domain, sub_mode, lifecycle, agency, scale, abstraction)`.
Each region's table is small and well estimated precisely because the 70-way
partition makes its traces homogeneous.

`ngram_table::build_structural` currently builds **one flat table**
(`physis-core/src/main.rs`). The per-cell hierarchy is **not built**. Do not
describe it as if it were.

---

## 3. The six operations

Every module is one of these. When asked "what does physis do", answer with this
table, not with one row of it.

| operation | modules |
|---|---|
| **assign** a coordinate | `classify` · `propose` |
| find where coordinates **fail** | `discovery` · `coverage` · `linkage` |
| find structure-preserving **maps** between regions | `transform` · `transplant` · `structsim` |
| detect **drift** vs. name collision | `becoming` |
| **record** assignments over time, and whether they held | `hypothesis` · `contradiction` · `epistemic` · `provenance` · `temporal` · `relation` · `explanation` · `delta_engine` · `ledger` |
| **select** which coordinates to read next | `rag` · `pack` · `map` |

Applications of the operations to a domain: 22 philosophical machines ·
industrial (`oee` `maintenance` `secom` `moonraker` `shifts` `report`) ·
backoffice (`reconcile` `inventory` `review_desk` `management_pack`) ·
knowledge (`wiki_gen` `corpus` `doctree` `communities` `decisions` `grounding`) ·
agent (`mcp` `ai/*` `routes` `extract`) · space·time·cause (`address` `ledger`
`mails` `audit` `operational`) · linguistic lenses (`wenyan` `piraha`
`sanskrit`) · personal (`life_os` `study` `interest` `monitor` `praxis`).

Counts: **physis-core 46 modules / 22.7k lines (Apache-2.0)** ·
**physis_pro 140+ modules / ~90k lines (proprietary)**.

---

## 4. The judge/propose distinction — do not get this wrong

The research track (`research/RESEARCH_STATUS.md`, E1–E18) refuted a long list of
mechanisms. **It refuted CERTIFICATION. It did not refute discovery, and it did
not refute proposal.** `physis-core/src/propose.rs` says so in its own header:

> All of those asked the machine to **judge**: *does this entry belong here?*
> This module asks it to **propose**: *which few cells should a person look at
> first?* Same geometry, different question, and the second one works.

| at 436 decisions seen | top-1 | top-3 | top-5 |
|---|---|---|---|
| `propose` | 0.447 | **0.712** | 0.791 |
| `old_cell`→`new_cell` table | 0.184 | 0.368 | 0.474 |
| majority cell | 0.089 | 0.255 | 0.359 |
| permuted-label null | 0.036 | 0.136 | 0.233 |
| entry length alone | 0.010 | 0.036 | 0.071 |

+0.531 top-3 over a construction-matched null. And the line to hold onto:
**top-1 ≈ 0.45 reproduces an accuracy this project had already recorded as a
failure. The geometry did not improve. Returning three candidates instead of one
is the entire difference.**

Writing "the grid is not a discovery mechanism" is the error this section exists
to prevent. It was written once, in `RESCOPE.md`, and struck.

---

## 5. Measured positives · settled negatives · never asked

**Beat a construction-matched null:**

| mechanism | evidence |
|---|---|
| `propose` | top-3 0.712 vs null 0.136 (+0.531) |
| `coverage` | duplicate control Welch *t* = +12.11 · 2000 → 141 |
| discovery ranking | P@25 0.480 vs 0.132 · exact *p* = 2.3e-5 |
| retrieval (semantic vs lexical) | `hit@5` 0.57 vs 0.14 (+0.43), `benchmarks/retrieval/` |

**Settled negatives — do not relitigate:** embedding geometry gives no stable
ontology structure (cross-embedder ARI ≈ 0.10) · no cheap signal *certifies*
structure (every scorer at chance on real misfilings) · cross-model disagreement
carries no epistemic signal (paired *t* = −10.53, 0/22) · the machine cannot
judge whether an entry *belongs* (AUC 0.410).

**Never put in front of a null:** ~~`transform`~~ (measured 2026-09-12, see
below) · `transplant` · `structsim` · `becoming`.

**`transform` — measured, positive, replicated (E53).** Link prediction over the
`crate::X` dependency graph, top-3 containment:

| corpus | ANALOGY | popularity | permuted null | random |
|---|---|---|---|---|
| physis-core/src (n=86) | **0.500** | 0.337 | 0.209 | 0.151 |
| physis_pro/src (n=294) | **0.446** | 0.303 | 0.256 | 0.061 |

+0.291 / +0.189 over the degree-preserving control. On the permuted graph
ANALOGY *loses* to popularity on both corpora — the arms behave as designed
rather than drifting together. Report: `physis-core/research/E53_TRANSFORM_ANALOGY.md`.

This licenses "measured against a control on **ranking**, positive". It does
**not** license calling `transform` a generator; that needs a gate-passing
output, which is the next experiment.

---

## 6. The neurosemantic expansion

Five agent harnesses, all local, all writing into **one** coherence graph and
**one** replayable `EpistemicAuditTrail` (`wiring/neurosemantic_wiring.md`):

| # | harness | on disk 2026-09-12 |
|---|---|---|
| 1 | **Hermes** — native `.mcp.json` stdio + skills | `~/.hermes` ✓ |
| 2 | **OpenCode** — `auth.json` providers, routes via openrouter/deepseek/groq/nvidia | `~/.config/opencode` ✓ |
| 3 | **Cline** — `cline-delegate` skill, delegates with physis coherence context | `~/.cline` ✓ |
| 4 | **Claude Code** — `claude-code` SKILL.md, anthropic provider | `~/.claude` ✓ |
| — | **Kilo** — a *provider* (`.env` key, via openrouter or direct), **not a harness** | no config dir |

Four harnesses, not five. `wiring/neurosemantic_wiring.md` numbers Kilo in the
list while its own parenthetical says "no dedicated skill — treat as provider".
Counted as a harness it inflates the trace-source count by 25%.

Interfaces: harness (`.loom` / `.mcp.json` stdio / CLI) = internal · MCP stdio =
primary external (local, no cloud, **30 tools**) · REST/`web_server` =
commercial/per-tenant.

**This is the trace source.** "Extraction from agentic workflows (local)" means
these five harnesses' traces, landing in the ledger, feeding the per-cell tables
of §2. `ledger.rs` holds ordered causal event sequences; `build_structural`
consumes documents. **Nothing connects them yet.** That join is the highest-value
unbuilt thing in the repository.

---

## 7. The recurring defect shape

Four instances found in one session. All identical: **declared, reachable, never
called.**

| symbol | state when found |
|---|---|
| `OnnxEmbedder` | exported from `lib.rs`, called by nothing — every benchmark number came from a lexical hash |
| `FitnessShifted`, `OutcomeObserved` | declared in the event enum, emitted by no code path (`packets/PH-017`) |
| structural table classifier | built its own `RandomProjectionEmbedder::new(64)`, so symbol sequences were noise with a schema |
| `build_structural` ← `ledger` | both halves exist, join absent |

Before claiming a capability, **grep for its call sites.** A module that compiles
and exports is not a module that runs. `coverage` already computes exactly this
shape over an ontology; pointed at the crate graph it is a self-debugger.

---

## 8. Rules for anyone summarising this repository

1. Do **not** reduce Physis to one noun. Use §1's sentence and §3's table.
2. Do **not** confuse *judge* with *propose* (§4).
3. Do **not** cite `benchmarks/ground-truth` as retrieval evidence — it scores
   identically under a lexical hash and a real model. Use `benchmarks/retrieval/`.
4. Do **not** claim a capability without grepping its call sites (§7).
5. Say **70 tables with subtables**, never "70 symbols" (§2).
6. When a measurement is absent, write `NOT MEASURED`. Never imply.

These rules are installed as an agent skill — `docs/PHYSIS_SKILL.md`, which is
the copy of record for `~/.claude/skills/physis/SKILL.md`. Mechanical checks:

| rule | check |
|---|---|
| declared ≠ called | `./scripts/declared-never-called.sh [dirs]` |
| does it discriminate? | `python3 benchmarks/retrieval/run.py --arms rp,semantic` |
| which module does X? | `docs/wiki/MODULE_MAP.md` (regen: `./scripts/gen-wiki.sh`) |
| already tried and failed? | `just recall "<task>"` |
