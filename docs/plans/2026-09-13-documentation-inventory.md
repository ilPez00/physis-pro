# Documentation inventory — what exists, what contradicts what

2026-09-13, second pass. The first pass (`2026-09-13-claims-audit.md`) checked
individual claims and was explicitly a sample. This one looks at the corpus as
a whole, because the reported problem was not "a claim is wrong" but "the
documentation is wrong and it costs us time".

**Scope, stated up front:** 301 `.md` files exist. This pass classified all of
them, and examined the root-level set (25 files) and the definition documents
in detail. It did **not** read all 107 prose documents individually. Where a
finding is mechanical (duplication, contradiction between named files) it was
measured; where it is not, it says so.

---

## 1. What the 301 files actually are

| class | count | what it is |
|---|---|---|
| fixtures / corpora | ~56 | `physis-core/examples/demo-corpus`, `benchmarks/ground-truth` — **test data, not documentation.** Should never be counted as docs |
| experiment records | ~45 | `research/mapper`, `research/impossible_machine`, `research/convergence` — write-once records of E-numbered experiments |
| work records | ~39 | `packets/` + `packets/reports/` — the board; a distinct genre with its own protocol |
| prose documentation | ~107 | `docs/` (41), root (25), `docs/business` (16), `docs/plans` (12), `physis-core` (8), `docs/archive` (5) |
| other | ~54 | `.opencode`, `.physis/wiki`, `wiring`, generated |

The first thing worth fixing is that these are not distinguished anywhere. A
reader — or an agent — walking `*.md` cannot tell a demo corpus from the README.

## 2. Five documents answer "what is Physis" and they do not agree

| document | answer |
|---|---|
| `README.md` | "the reasoning core of the Ayu / Praxis ecosystem" |
| `physis_bible.md` | same sentence (it contains the README) |
| `docs/WHAT_PHYSIS_IS.md` | "a **coordinate system for meaning** — 5 domains × 14 modes = 70 cells … plus the operations that assign, revise and audit coordinates" |
| `RESCOPE.md` §0 | "an **epistemic ledger** for agent work" |
| `PLAN.md` | "the layer both [LangChain + Semantic Router] …" |

`docs/WHAT_PHYSIS_IS.md` is the canonical one, and it exists *because* of this
problem. It opens:

> This file exists because three separate attempts in one session compressed a
> 190-module, ~113k-line system into a single noun, and all three were wrong.

and it lists attempt 2 — *"No — an epistemic ledger"* — as **wrong**. That is
the definition `RESCOPE.md` §0 still leads with. RESCOPE does carry a note
saying it is superseded as a definition, but RESCOPE is 588 lines, is the most
cited document in the repository, and was cited repeatedly as authoritative
throughout this session, superseding note included.

**This is not a tidiness problem.** It is why a careful reader forms the wrong
model of the system and then writes it into new documents — which is exactly
what happened twice today.

### And the canonical definition rests on the disputed component

The one true sentence is built on the 70-cell grid. Conceptual problem 3
(`2026-09-13-four-conceptual-problems.md`) is that the grid is the single
belief in the engine that no evidence can revise — while E5 found that
collapsing 57 → 14 classes *gains* kappa, and E8's pre-flight STOPs on sparsity
at every level.

So the canonical answer to "what is Physis" is defined in terms of the
component its own research most disputes. That tension is recorded in neither
document.

## 3. `physis_bible.md` is a stale concatenation, and it is 47,492 lines

Measured overlap, by identical unique lines:

| document | % of it that is inside the bible |
|---|---|
| `README.md` | **99%** (635 of 639 unique lines) |
| `PHYSIS_THESIS.md` | **98%** |
| `AGENTS.md` | **88%** |
| `PLAN.md` | 58% |
| `docs/WHAT_PHYSIS_IS.md` | 4% |
| `TESTING.md` | 3% |
| `RESCOPE.md` | **1%** |

The pattern is unambiguous: the bible was assembled from the documents as they
stood at some past moment, and the documents written since — RESCOPE,
WHAT_PHYSIS_IS, TESTING — are absent. A reader who consults the "bible"
because the name implies canonicity gets a stale snapshot, including the
definition that WHAT_PHYSIS_IS was written to retract.

This is the **fourth** instance today of one defect shape: `rag.rs` in two
crates (PH-012), the structural machines in library and example (PH-021),
`physis-check.sh` in two repositories where the shipped copy was the broken one,
and now the documentation itself. Copy, drift, and the copy that drifts is the
one someone reads.

## 4. What should change

Not a rewrite. Four mechanical moves, in order of value:

1. ~~**Delete `physis_bible.md`, or generate it.**~~ **Done 2026-09-13 —
   deleted.** 47,492 lines removed. It was a hand-assembled snapshot containing
   99% of README, 98% of PHYSIS_THESIS, 88% of AGENTS and 58% of PLAN, while
   missing RESCOPE (1%) and WHAT_PHYSIS_IS (4%) — the two documents written to
   correct what it still asserted. Recoverable from git history at `f726e7a` if
   the bundle is ever wanted; if it is, it must be **generated by a command**
   and never hand-edited, or it drifts again.

   Independent corroboration that it was already a maintenance liability:
   `packets/reports/PH-002.md` records having to fix the same stale path in
   "3 spots of `physis_bible.md` (a rolled-up copy)".
2. **One definition, referenced everywhere else.** `docs/WHAT_PHYSIS_IS.md`
   already holds the job. **Partly done 2026-09-13:** `README.md` now opens by
   naming the canonical file and says explicitly that its own paragraph is an
   orientation, not a definition, and that the canonical file wins any
   disagreement. `RESCOPE.md` §0 now carries a note that the paragraph below it
   is **wrong attempt 2** in the canonical file's own list — kept rather than
   deleted, because removing a retracted definition hides that it was ever
   believed, and it was believed all of 2026-09-13 by an agent citing it.

   **Still open:** `PLAN.md` states a fourth definition and is untouched.
3. **Separate the genres in the tree.** Fixtures are not documentation and
   should not answer to `find . -name '*.md'`. `packets/` and `research/` are
   records with their own protocols. A `docs/` that contains only prose
   documentation is navigable; 301 undifferentiated files are not.
4. **Make the definition falsifiable like everything else.** If the 70-cell
   grid is the definition, and E5 says 14 classes score better, then either the
   definition or the grid is a claim with evidence against it. Today it is
   neither — it is prose, exempt.

## 5. What this pass did not do

- Did not read the 41 files in `docs/`, the 16 in `docs/business`, or the 5 in
  `docs/archive` individually. Overlap and contradiction between *named* files
  was measured; unexamined files may hold more.
- Did not check `research/` records against their own code. Those are
  write-once experiment records and a different audit.
- Did not verify the `docs/business` numbers at all. They are the ones most
  likely to be quoted outward and least likely to have been re-run.
