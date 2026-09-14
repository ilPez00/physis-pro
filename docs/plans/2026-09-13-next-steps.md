# Next steps — the neurosemantic interface

Written 2026-09-13, after a literature pass (searches logged in §1) run against
the state left by `docs/plans/2026-09-13-research-agenda.md`. That file is the
list of open *experiments*. This file is the list of what to *do next*, in
order, with the experiments folded into it at the points where they gate a
build decision.

The organising claim, unchanged from `computer-remake-research/notes.md`:

> Three layers — OBSERVE (append-only), BELIEVE (claims + evidence + replay),
> ACT — with the model **outside** as a replaceable interpreter.

"Remake the computer" is that architecture applied to the machine itself rather
than to a document corpus. Everything below is scored against one question:
*does this move a layer from "measured in a corpus" to "running on the box"?*

---

## 0. Standing rules for every item here

Inherited from the research agenda and from `docs/WHAT_PHYSIS_IS.md` §8; they
are restated because most of the failures in this repository were caused by
skipping one of them.

1. Every item names its control. No control run ⇒ not a result.
2. Before claiming a capability, `./scripts/declared-never-called.sh` it.
3. `NOT MEASURED` is a legal value. Implication is not.
4. A comment recording a failure gets **updated**, never deleted.
5. Numbers not regenerable by a command in this repository get flagged, per
   `docs/plans/2026-09-13-claims-audit.md`.

---

## 1. Literature delta, 2026-09-13 (second pass)

The first pass (agenda §"The reading list") predates the papers below. Four of
them change what is worth building; two close an open question outright.

| paper | id | what it changes here |
|---|---|---|
| **MemStrata — Temporal Validity in Retrieval Memory** | arXiv 2606.26511 | **Closes the design half of problem C.** It is the (subject, relation, object) supersession tuple over a bi-temporal ledger, deterministic, no similarity threshold, no LLM call at supersession time. Reports cosine separating a contradicted fact from a duplicated one at **AUROC 0.59** — independent replication of C2's finding that the embedding cannot tell "same" from "opposite". Reports 0.95–1.00 on evolving knowledge where RAG gets 0.20–0.47, on a 7B local model on consumer hardware. |
| **Temporal Validity on Real Software Histories** | arXiv 2608.20685 | The same design validated on 130 atomic state transitions mined from 707 real GitHub issues: 0.91 vs RAG 0.57–0.59, RAG serving the superseded value 36–38% of the time when forced to answer. This is the *corpus construction protocol* physis should copy for its own ledger. |
| **PersistBench** | arXiv 2602.01146 | "When should long-term memories be forgotten" — public-looking benchmark in the STALE family; check for a dataset before authoring another synthetic corpus. |
| **MemTX — transactional belief commit** | arXiv 2607.23929 | Belief commit as a transaction. Directly relevant to the AGM shape gate in ROADMAP Phase 22 (A3). |
| **Revoked but Still Authoritative** | arXiv 2609.08258 | Empirical study of revocation *enforcement* in agent-memory systems — i.e. the failure mode where invalidation is recorded and then ignored downstream. This is Gap 11 with a measurement protocol. |
| **When Stale Constraints Go Unchecked** | arXiv 2608.25553 | Budgeted verification failures in inherited agent memory. |
| **Always-On Agents (survey)** | arXiv 2606.30306 | Persistent memory / state / governance survey; the map for anything in Phase 22–23. |
| **From Agent Traces to Trust (survey)** | arXiv 2606.04990 | Evidence tracing and execution provenance — the field name for `provenance.rs` + the hash chain (A4/G4). |
| **DMI — From Imperative to Declarative OS interfaces** | arXiv 2510.04607, EuroSys'26 | GUIs recast as three declarative primitives — **access, state, observation**. +67% task success, −43.5% steps, >61% of successes in a single LLM call. This is the ACT layer's published prior art and it is a better shape than "drive AT-SPI actions imperatively" (Gap 10's proposal). |
| **A11y-CUA** | arXiv 2602.09310 | Characterises the accessibility gap in computer-use agents — the evidence for how often the a11y tree is *missing or wrong*, which is the failure mode a physis observer must record rather than paper over. |
| **UFO2 — The Desktop AgentOS** | arXiv 2504.14603 | The nearest existing "agent OS"; inventory it before building an observer, it is not in `computer-remake-research/inventory.md`. |
| **IOF Core** (Industrial Ontologies Foundry, BFO-based) | spec.industrialontologies.org, github.com/iofoundry/Core | **The domain reference D1 is missing.** BFO-tiered, manufacturing mid-level, OWL — exactly what DeepOnto is the harness for. |

**Still blocked:** STALE (arXiv 2605.06527) has no findable dataset or code
repository as of this pass. Recheck monthly; `MemoryAgentBench`
(github.com/HUST-AI-HYZ/MemoryAgentBench) remains the public substitute.

**Reading order for the next work session:** 2606.26511 → 2608.20685 →
2510.04607 → IOF Core spec. Everything else is reference.

---

## 2. Track NS — the neurosemantic substrate (the "remake the computer" leg)

This is the track that has no experiment blocking it and the largest unbuilt
surface. `docs/WHAT_PHYSIS_IS.md` §6 names its first item as *"the
highest-value unbuilt thing in the repository"*, and it is still unbuilt:
`ngram_table::build_structural` has exactly three call sites
(`physis-core/src/main.rs:1902`, its own definition, one test) and none of them
is the ledger.

### NS1 — Join the ledger to the structural table

**What.** `build_structural` consumes documents; `src/ledger.rs` holds ordered
causal event sequences. Make the ledger a document source, so the per-cell
tables of `WHAT_PHYSIS_IS` §2 are estimated from *traces of work* rather than
from ontology seed text.

**How.** A `LedgerCorpus` adapter emitting one segment per causal chain, with
the cell coordinate already assigned by the event, so `classify` is not in the
loop. Keep it in physis-core behind the existing `RunConfig` so the table
backend stays swappable.

**Control.** The table built from the ledger must beat the table built from the
same number of *shuffled* ledger events (order destroyed, cell marginals kept)
on next-cell prediction. If order carries nothing, this join buys nothing and
must be reported as such — that is the whole point of building it against a
null rather than shipping it.

**Second control.** Compare against the current flat table on the same held-out
tail. §2 predicts per-region conditioning wins because the 70-way partition
makes traces homogeneous; that prediction is falsifiable and has never been put
in front of a number.

**Artifact.** `benchmarks/results/ledger-structural.json`, plus a row in
`RESCOPE.md` §1 or an explicit `NOT MEASURED`.

**Cost.** One week. **Kill criterion:** shuffled control within noise ⇒ write it
down in `ngram_table.rs`'s header as a measured negative and stop.

### NS2 — The observation layer on the actual machine

Gap 1 (one substrate, many kinds of thing) and Gap 10 (applications as
machine-readable state) are the two the survey says nobody holds together.

**What.** One append-only observer writing `Event {timestamp, duration, data}`
— ActivityWatch's primitive, which the survey found to be the only
domain-neutral one — from three sources on this box:

1. filesystem events (already partly present via the watcher that hit the
   500k-observation cliff; reuse the bounded tail read),
2. **AT-SPI2 accessibility tree** snapshots — state, not pixels, no OCR,
3. harness traces from the four confirmed harnesses (`~/.hermes`,
   `~/.config/opencode`, `~/.cline`, `~/.claude`) — four, not five; Kilo is a
   provider (`WHAT_PHYSIS_IS` §6).

**Budget, stated as a requirement before a line is written**, because Gap 6 says
this is what kills semantic desktops: <10% of one core, <500 MB RSS, sustained,
with a `monitor_resource.sh` run of ≥24 h as the acceptance evidence. Nepomuk
did not die of bad semantics.

**Control for the a11y leg.** A11y-CUA (arXiv 2602.09310) says the tree is often
absent or wrong. Sample N windows, compare the tree's claimed controls against
what is actually there, and **record the miss rate as a first-class number**. An
observer that cannot report its own blindness is Gap 3 at the OS level.

**Cost.** Two weeks for the skeleton plus the 24 h budget run. **Kill
criterion:** budget breach that is not fixable by sampling rate ⇒ drop the
continuous observer, keep event-driven capture (screenpipe's own retreat).

### NS3 — ACT as declarative primitives, not synthesised clicks

**What.** Do not build an imperative AT-SPI driver. Build DMI's three primitives
(arXiv 2510.04607) over what NS2 already observes: **access · state ·
observation**, with `act.rs` as the belief-side gate in front of them.

**Why this is the physis version and not a reimplementation of DMI.** DMI makes
agents efficient. It has no memory of what it has already established, so it
cannot answer Gap 8 — *did what I just did contradict something I already
established?* Physis's contribution is that every declarative action is
recorded as an observation, every intention as a claim, and `act::bearing_on`
runs the cascade (agenda item A) + the 0.66 floor (item B) over the ledger
before the action executes.

**Control.** The DMI numbers are the baseline to beat or match on step count;
the physis-only column is *contradiction warnings that were correct*, scored
with the same false-alarm accounting item B used (harm removed per alarm). A
gate that fires on nothing is free and useless; a gate that fires wrongly costs
2.00 harm per alarm by that scale.

**Cost.** Three weeks, and it depends on NS2. **Do not start before NS2's
budget run passes.**

### NS4 — Absence and deletion as facts (Gap 11)

`invalid_at` exists in graphiti and only over LLM-extracted facts; the survey's
own experiment found it is never *narrowed* when later evidence moves the
boundary earlier. `TemporalValidity` + `narrow_until` already exist in physis
(agenda D, "the mechanism physis already has"). Wire the observer's deletion
events into them, and score against **Revoked but Still Authoritative**
(arXiv 2609.08258) — whose whole point is that recording a revocation and
*enforcing* it downstream are different, and systems fail the second.

**Control.** Query the ledger after a revocation and count how often the
retracted value is still served. The paper's protocol gives the shape.

---

## 3. Track C — claim identity, now largely settled by MemStrata

**Status change.** C's open item was *"the tuple itself … needs an extractor,
which needs an interpreter"*. MemStrata (2606.26511) publishes that tuple, with
a deterministic supersession rule, and reports the same embedding failure physis
measured independently in C2. **Physis should replicate, not design.**

### C4 (new) — implement the supersession tuple in `claim_identity.rs`

`physis-core/src/claim_identity.rs` already has the structural identity — bag of
content words + scoped polarity — measured at **precision 1.000, recall 0.375**
(C2). Its stated ceiling is that a bag has no subject/object asymmetry and no
interval. That is exactly what (subject, relation, object) + bi-temporal
interval supplies.

**Steps.**

1. Extend the identity to `(subject, relation, object, valid_interval)`.
   Extraction by interpreter, per C2's note; supersession itself stays
   deterministic and model-free, which is MemStrata's load-bearing choice.
2. Re-run `examples/claim_identity` on the same 64-claim / 2016-pair corpus.
   **The corpus must keep the eight agreeing restatements** — C2's first attempt
   was discarded for lacking them, and without near-duplicate agreements as
   negatives the measurement is meaningless.
3. Report precision/recall/F1 against three arms: `embedding@0.90` (0.667 /
   1.000), `structural@0.30` (1.000 / 0.375), and the new tuple.
4. Add MemStrata's own reported numbers to the table as an external anchor, not
   as a physis result.

**Falsifier.** If the tuple does not raise recall above 0.375 while holding
precision at 1.000, the parser was not the missing piece and C2's argument was
wrong. Record it in the module header.

**Corpus upgrade, separately.** Copy 2608.20685's construction: mine atomic
state transitions from real GitHub issues in this repository's own history
rather than authoring synthetic ones. That converts C from a synthetic result
into a real one and is the single highest-credibility move available on this
track.

### C3 — finish the interpreter swap

`research/c3_interpreter_swap.py` is mid-flight; the first result was withdrawn
for the NIM `reasoning_content` harness bug. Finish the re-run. Both readings
are already written down in the agenda, including the one where graphiti's
stability turns out to be graphiti's *normaliser* rather than the models'. If
that is what lands, the work item becomes **build the normaliser**, and it
belongs in `claim_identity.rs` next to C4.

### C1 — STALE watch

One search per month. Unblocks a whole external-validity column when it lands.
Substitute: MemoryAgentBench, and now PersistBench (2602.01146) — check that
one's availability in the same pass.

---

## 4. Track D — ontology as proposal

D1, D2a and D4 ran on 2026-09-13. What is open is stated precisely in the
agenda; here is only what to do next and in what order.

### D1b — re-run the anchor against a *domain* reference

The finding "62% of the grid is disputed" is against **WordNet supersenses** — a
lexical carve of common nouns — and the agenda itself says the protocol is the
result while the number needs a domain reference before it is quotable.

**The reference to use: IOF Core** (Industrial Ontologies Foundry, BFO-tiered,
OWL, `github.com/iofoundry/Core`, `spec.industrialontologies.org/iof/`). It is
mid-level manufacturing, which is the domain the industrial cells actually live
in, and it is OWL — which is precisely the condition under which the agenda says
**DeepOnto stops being optional** ("the moment the anchor is SUMO, DOLCE or
schema.org, the alignment stops being a lookup and becomes matching"). Add
schema.org as the cheap second reference for the backoffice cells.

**Protocol.** OAEI metrics — precision, recall, F, MRR, Hits@K, **coherence** —
plus the OAEI-LLM-T error typology, and report the **disputed** count as the
headline. Keep D1's two corrections: permutation null instead of a fixed
concentration threshold, and no hypernym-hop dilution.

**Rule.** DeepOnto is a Python dependency. It lives in `research/`, never in the
crate.

**Falsifier, restated because it is the one that matters.** If disputed is near
zero against IOF, the grid is a relabelling of a standard and the interesting
claim is gone. That must be reportable.

### D2a-b — the mode axis against a *domain* verb inventory

Same move on the verb side. WordNet's fifteen verb supersenses said eight of
fourteen modes carve, `CREATE` carves nothing while `verb.creation` sits
unowned, and `verb.social` (12.2% of all verb uses) is the largest hole. A
domain inventory — IOF process terms, or VerbNet/FrameNet/PropBank frames
restricted to the corpus's verbs — is what distinguishes "`verb.possession` is
irrelevant to an industrial backoffice" from "it is a real gap". WordNet cannot.

### D2 — proposals from the corpus, first target already known

`BOND/MAINTAIN` (n=15, fitness 0.067 against a null of 0.015) from D4, and
`FABRICATE` ↔ `CONSTRUCT` from E22, are the two worked examples. If the
proposal pipeline cannot recover a merge the evidence already plainly supports,
it will not find a subtler one — run those two before generalising.

### D3 — the approval loop, with its own agreement measured

Unchanged: route each proposal to a human and to a model independently and
measure inter-annotator agreement **before either is trusted** (the 0.97 vs 0.68
result says neither is an oracle). Gate: if the two adjudicators disagree more
than the proposals vary, the loop is a coin flip with a person in it — report
that and stop.

**Nothing in D edits the grid.** Every output is a proposal with a validity
window in `hypothesis.rs`. Approval is supporting evidence; rejection is
contradicting evidence.

---

## 5. Track X — external validity, because everything above is self-scored

Four of the five measured positives in `WHAT_PHYSIS_IS` §5 are against
physis-authored corpora with physis-authored nulls. The nulls are honest and the
corpora are small. One external number changes the character of the whole
claim.

Priority order, by availability:

1. **MemoryAgentBench** (public) — the memory track's substitute for STALE.
2. **PersistBench** (2602.01146) — check availability this pass.
3. **OAEI tracks** — for D1b, once the anchor is OWL.
4. **SWE-bench-derived transition mining** — per 2608.20685, for C4's corpus.
5. **STALE** — when released.

Report the external number *beside* the internal one, never instead of it, and
keep STALE's headline (best evaluated model at **55.2%** overall) as the
context line for anything physis claims about acting on updated evidence.

---

## 6. Sequencing

Two lanes, because NS is build work and C/D are measurement work and they do
not contend for the same hours.

| week | NS lane (build) | C/D lane (measure) |
|---|---|---|
| 1 | NS1 join + shuffled control | C3 re-run finishes; C4 tuple design against 2606.26511 |
| 2 | NS2 observer skeleton, 24 h budget run | C4 implemented, re-run on the 2016-pair corpus |
| 3 | NS2 a11y miss-rate control | D1b: IOF + DeepOnto harness up |
| 4 | NS3 DMI primitives (only if week 2 budget passed) | D1b run; disputed count against IOF |
| 5 | NS3 + `act` gate, false-alarm accounting | D2a-b domain verb inventory |
| 6 | NS4 revocation enforcement, scored per 2609.08258 | D2 on `BOND/MAINTAIN` and `FABRICATE`↔`CONSTRUCT` |

C4's real-history corpus (from 2608.20685's protocol) is the first thing to pull
forward if a lane frees up. D3 is scheduled only once D2 has emitted proposals
to adjudicate.

---

## 7. The search queue for the next literature pass

Run these; they are the ones this pass left open.

1. `STALE 2605.06527 dataset` / `PersistBench dataset` — monthly, until one lands.
2. TOKI (2606.26511 is *not* TOKI — TOKI is 2606.06240) numeric tables: the PDF
   extraction failed here, so its measurements remain uncited. Get them from the
   HTML version before citing a number.
3. `Negation is not semantic` (2603.17580) — same problem, same fix.
4. UFO2 (2504.14603) and DMI (2510.04607) into
   `computer-remake-research/inventory.md`; neither is in the 21-repo survey,
   and the survey's Gap 10 verdict ("exactly one project uses the accessibility
   tree") was derived without them.
5. OAEI 2026 track list — whether any track matches an industrial mid-level
   reference, which would give D1b a published baseline instead of only a null.
6. IOF Core release state — the Core v1 beta noted in the NIST paper may have
   advanced; check `github.com/iofoundry/Core` tags before pinning a version.

---

## 8. What this plan does not do

- It does not touch the core/Pro licensing boundary. That was item E, and it was
  dropped from the agenda as a business decision, not an experiment. It stays
  dropped here.
- It does not reopen the settled negatives (`WHAT_PHYSIS_IS` §5): embedding
  geometry as ontology structure, cheap certification, cross-model disagreement,
  belongs-here judgement, or the warning reserve.
- It does not promise the per-cell hierarchy of §2 works. NS1 is the experiment
  that asks.
- It does not call `transform` a generator. That still needs a gate-passing
  output (E53's stated next step) and it is not scheduled here.
