# What Physis is

**Read this before summarising Physis to anyone, including yourself.**

This file exists because three separate attempts in one session compressed a
190-module, ~113k-line system into a single noun, and all three were wrong:

| attempt | claim | why it was wrong |
|---|---|---|
| 1 | "Physis is a context compiler." | Context compiling is one operation out of six. MIT alternatives beat it on token reduction. |
| 2 | "No — an epistemic ledger." | The ledger is a second operation out of six. |
| 3 | "70-symbol vocabulary, one n-gram table." | It is **tables with subtables** — 45 under the default pack. See §2. |

Each attempt sounded sharper than the last and was equally wrong. **The system
does not compress to a noun. Describe it structurally.**

---

## 1. The one true sentence

> Physis is a **coordinate system for meaning** — *n* domains × 9 modes,
> tree-nested by facets — **plus the operations that assign, revise and audit
> coordinates**, plus the applications built on those operations.

**Revised 2026-09-14: the domain axis is open, and no count was ever measured as
a law.** **Revised 2026-09-19: the mode axis was reworked from fourteen modes to
nine**, each named for the WordNet verb supersense it owns (D2a) — see §2. The
default pack is now 5 × 9 = 45 cells, and `PHYSIS_DOMAINS` replaces the domain
list per deployment (`src/semiotic_grid.rs`, `domains()` / `cell_count()`).

Everything else is a consequence of that sentence.

---

## 2. The coordinate system

```rust
DEFAULT_DOMAINS: [HEAL, CONSTRUCT, FABRICATE, BOND, STUDY]   // the default pack
MODES:           [COMMUNICATION, COGNITION, MOTION, REST, MAINTAIN,
                  SOCIAL, CREATE, POSSESS, STATIVE]                      // 9
```
`src/semiotic_grid.rs` — **`domains().len()` × 9 cells; 45 with the default
pack.** Each mode owns one supersense and `MODE_OWNERS` carries the measurement
that put it there. The domain axis is content-side (*what the work is about*) and is
replaceable via `PHYSIS_DOMAINS`; the mode axis is operative (*tool use, method,
workflow*) and is not currently overridable.

**Why the counts are a pack and not a law (E66, 2026-09-14).** Both axes were put
in front of D4's size-preserving permutation null, same 731 entries, same
embeddings:

| axis | classes | self-recovery | null | normalised (f−n)/(1−n) | classes failing |
|---|---|---|---|---|---|
| DOMAIN/MODE | 70 | 0.438 | 0.038 | 0.416 | 8 |
| DOMAIN only | 5 | 0.672 | 0.209 | **0.585** | 0 |
| MODE only | 14 | 0.536 | 0.121 | 0.472 | 0 |

**Both axes carve — every class on both clears its own null**, so neither is
merely written down, and the domain axis is if anything the *more* recoverable
of the two. What no row supports is *five* or *fourteen*. External anchors cut
the other way: 35 of 70 cells are `disputed` against WordNet noun supersenses
(D1), and 6 of 14 modes fail against its verb supersenses while `verb.social`
— 12.2% of the corpus's verb mass — is owned by no mode (D2a). Internal
coherence says the axes are real; external anchors say the *counts* are
convention.

**Claim boundary — that table is the PRE-REWORK axis.** Every number in it was
measured on the fourteen-mode axis that existed on 2026-09-14. It is the
*evidence for* the rework below, not a measurement of it. The nine-mode axis is
**not yet measured**: its self-recovery, its per-class nulls and its cell
disputes are all unknown until the same 731 entries are put through the same
permutation null again. Do not quote 0.416 / 0.585 / 0.472 as properties of the
current grid.

**The rework (2026-09-19), from that evidence.** D2a measured every mode against
WordNet's verb supersenses. Acting on it, the mode axis is nine modes:

- **RETIRE** — LIFT, SENSE, WALK, BRAINSTORM, DESTROY and the old CREATE did not
  clear their own null. The old CREATE is retired as a word and re-anchored:
  its dominant supersense was `verb.contact`, not creation.
- **MERGE** — LEARN+PLAN → COGNITION, GUIDE+WORK → COMMUNICATION, MOVE+PLAY →
  MOTION. Each pair shared one dominant supersense, so the axis carried the same
  distinction twice.
- **EXPAND** — SOCIAL (`verb.social`, 12.2% of the corpus's verb mass and
  unowned), CREATE (`verb.creation`, 7.9%), POSSESS (`verb.possession`, 5.6%),
  STATIVE (`verb.stative`, 5.3%).

Hence: domains open, modes **fixed at nine, each with an owner**. A mode that
owns nothing is a word the corpus has no use for.

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

**So the unit is not "70 symbols". It is 45 tables, each with subtables.** A flat
symbol vocabulary models `P(next cell | previous cells)` and throws the facets
away. The architecture is a *conditional model per region*:
`P(next | cell, sub_domain, sub_mode, lifecycle, agency, scale, abstraction)`.
The hypothesis is that conditioning could yield more homogeneous traces.
Sample sufficiency and improved estimation must be measured; partitioning alone
does not establish either property.

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
| **select** which coordinates to read next | `rag` · `pack` · `map` · `system` |

Applications of the operations to a domain: 22 philosophical machines ·
industrial (`oee` `maintenance` `secom` `moonraker` `shifts` `report`) ·
backoffice (`reconcile` `inventory` `review_desk` `management_pack`) ·
knowledge (`wiki_gen` `corpus` `doctree` `communities` `decisions` `grounding`) ·
agent (`mcp` `ai/*` `routes` `extract`) · space·time·cause (`address` `ledger`
`mails` `audit` `operational`) · linguistic lenses (`wenyan` `piraha`
`sanskrit`) · personal (`life_os` `study` `interest` `monitor` `praxis`).

The **select** row is the one an outside caller touches first.
`system` (Pro's `src/ported/system*.rs`, reusing Core primitives;
`docs/SYSTEM_INTERFACE.md`) is that row made addressable: one
`Workspace` service — `inspect · list · find · pack · read · history · remember ·
run · export` — behind a CLI, a versioned JSON contract, a folder snapshot and a
terminal UI. `pack` is `rag`'s token-fixed retrieval applied to a workspace: it
ranks 40-line windows and fills a token budget, so the answer is the context
rather than a pointer to it. Measured against the shell an agent would otherwise
run, that is 6051 tokens at 5/5 answers-in-context against 14179 at 5/5 for
`grep … | head` then `cat`.

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

The historical wiring inventory lists four harnesses and one provider
(`wiring/neurosemantic_wiring.md`). Installed configuration alone does not prove
that all harnesses write to one store; verify active connections:

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

**These are intended trace sources.** `ledger.rs` holds ordered causal event
sequences; `build_structural` consumes documents. The trace-to-per-cell-table
join is not established here. Its priority is a product/research decision,
not an architectural fact. The [impossible computer programme](plans/2026-09-19-impossible-computer.md)
prioritizes a usable action/review/outcome loop before this experiment.

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
5. Say **tables with subtables — 45 under the default pack**, never "70 symbols",
   and never "the five domains" as if five were measured (§2). The mode axis is
   nine modes, each owning one verb supersense; the pre-rework measurements are
   evidence for that rework, not measurements of it.
6. When a measurement is absent, write `NOT MEASURED`. Never imply.

These rules are installed as an agent skill — `docs/PHYSIS_SKILL.md`, which is
the copy of record for `~/.claude/skills/physis/SKILL.md`. Mechanical checks:

| rule | check |
|---|---|
| declared ≠ called | `./scripts/declared-never-called.sh [dirs]` |
| does it discriminate? | `python3 benchmarks/retrieval/run.py --arms rp,semantic` |
| which module does X? | `docs/wiki/MODULE_MAP.md` (regen: `./scripts/gen-wiki.sh`) |
| already tried and failed? | `just recall "<task>"` |
