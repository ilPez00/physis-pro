# Claims audit — what the docs say against what the tree contains

2026-09-13. Prompted by the observation that much of the documentation is
wrong and that this is costing real time. It is, and the count is worse than
it looks: **this session alone produced or exposed ten documentation errors,
five of them written by me during it.**

Each row below was checked with a command, not by reading. Where a claim is
correct it says so — an audit that only finds faults is not an audit.

---

## 1. RESCOPE §1 "Shipped and measured"

That table is explicitly a claims-with-evidence table, and RESCOPE's own
preamble says: *"Every number here was produced by a command in this
repository during the pass that wrote it."* That is the standard it asked to
be held to.

| Claim | Verdict | Evidence |
|---|---|---|
| Token-fixed retrieval, `TokenFixedRetriever` | **holds** | exists and is the shipped path |
| `same corpus ⇒ same structure_hash` | **holds** | `map.rs:513` asserts it |
| Discovery *filtering*, Welch *t* = +12.11 | **holds** | 7 in-tree hits; regenerable |
| Discovery *ranking*, precision@25 = 0.480 | **holds** | 4 in-tree hits; regenerable |
| Agent surface — "30 MCP tools over stdio" | **holds, exactly** | 30 distinct `physis_*` names in `src/mcp.rs` |
| "physis-core 194 pass" | **stale** | actually **270**. Understates, so it misleads nobody about quality — but a number nobody re-ran is a number nobody is checking |
| "clippy clean at `-D warnings`" | **wrong as stated** | see §2 |
| Context compression "253 → 165 tokens" | **not regenerable** | `253` appears in no `.rs`, `.py` or `.sh` in the tree |
| n-gram "338k lookups/s" | **not regenerable** | `338` appears nowhere in the tree |

The last two are the ones that matter, because they violate RESCOPE's own
preamble. A number that no command in the repository reproduces is a number
that cannot be checked, and the whole point of that table is checkability.
They may well be true. Nobody can tell.

## 2. "clippy clean at -D warnings" — precisely

`packets/README.md` states: *"`cargo clippy --all-targets -- -D warnings` is
CI. A warning is a build failure."* Measured now:

| target | errors |
|---|---|
| physis-pro, `--all-targets` | **0** |
| physis-core, `--lib --tests` | 2 (both `src/notebook.rs`, pre-existing) |
| physis-core, `--all-targets` | 8, **all in `examples/`** — 16 findings in `experiment49`, 4 in `experiment48`, plus `mapper_wordnet_export` and `experiment53` |

So the claim is true for physis-pro, true for physis-core's `src/`, and false
for physis-core as a whole. The honest form is "clippy clean in `src/`;
`examples/` is red and has been". Earlier notes in this session said "31
pre-existing errors"; the count moves with compilation order and caching, which
is itself a reason to state the *location* rather than the count.

## 3. Capabilities that are listed but unreachable

Run: `bash scripts/physis-check.sh sweep src physis-core/src` → **523 items**.
The tool is careful that each line is a question rather than a defect, so these
were confirmed by reading.

| Item | Finding |
|---|---|
| `transplant.rs` | **A complete island.** Zero uses anywhere — not `src/`, not the CLI, not examples, not tests. `RESCOPE.md:494` lists it as a capability under "Invariance & structure". RESCOPE:579 is honest that it "[has] never been put in front of a null"; what neither says is that nothing calls it at all |
| hybrid BM25 retrieval | **Ships disabled, and `lib.rs:78` says so** — the fused path failed to beat the cosine baseline, and the shipping decision was derived from the measurement. `measure_hybrid_vs_cosine` is called only from `#[cfg(test)]`. The code and `lib.rs` are honest here. **`computer-remake-research/inventory.md` was not** — it lists "hybrid cosine + BM25" as a shipped retrieval capability. That is my error, not the repo's |

## 4. The five errors this session produced or exposed

Recorded because the pattern matters more than any single item.

1. **"physis lacks bi-temporality, take it from graphiti"** — false. It has had
   both legs since G1 and `is_valid_at` reads all three. I read module docs
   instead of `temporal.rs`. *Corrected in `inventory.md`, `notes.md`.*
2. **"no numeric or time-series primitives"** — false. The Impossible Machine,
   KAIROS's scaling exponent, and E51/E52's frozen held-out splits with
   pre-registered gates and independence nulls are exactly that. Caught by the
   user, not by me. *Corrected in the markets note, in place, with the error
   left visible.*
3. **PH-021's gate said `cargo build --examples`** — can never pass; `main` in
   that example is behind `#[cfg(feature = "embed-onnx")]`. Verified on an
   unmodified checkout before blaming my own change. *Corrected in the packet.*
4. **`act.rs` said its matcher is "weak"** — true but unquantified. The 2×2
   measured that family at 0–1/7 against a 5/7 ceiling. *Now stated, with the
   structural consequence.*
5. **PH-018 was red on `main`** and the packets said so, but nothing surfaced
   it: a deterministic failure reproducing in 2.3 s sat inside a 235 s suite
   that runs once per packet. *Fixed; the visibility problem is recorded in the
   report.*

## 4b. The checking tool is itself duplicated, and the copies have drifted

Found while looking for `physis-check` to run this audit. It exists twice:

| copy | lines | `verdict` |
|---|---|---|
| `physis-pro/scripts/physis-check.sh` | 344 | **fixed** today (`main`) |
| `ilPez00/physis-skill` `scripts/physis-check.sh` | 319 | **still broken** |

27 diff lines between them, and the substance is one function. `physis-skill`
is the *distributable* — `install.sh:40` copies `physis-check.sh` to wherever an
agent installs the skill — so the broken copy is the one that actually reaches
agents.

The bug is that `verdict` silently records nothing. From the fix's own comment
in the repaired copy:

> without `PHYSIS_ALLOW_DEV_LICENCE` a dev build exits 2 on the licence gate,
> `|| true` swallowed it, and `verdict` printed nothing and exited 0. **A
> verdict that silently does not record is worse than none — the next session
> repeats the work believing it was logged.**

This is the audit's thesis in its sharpest form. The tool whose entire purpose
is to stop an agent claiming what it did not verify **silently fails to
record**, in the copy that ships, while the fixed copy sits in a different
repository. It is also the third instance today of one defect: `rag.rs` in two
crates (PH-012, where the MMR fix reached one copy and not the other), the
structural machines duplicated between library and example (PH-021, avoided on
purpose), and now this.

Not fixed here: `physis-skill` is a separate repository and was not in scope
for this session. It should be the next thing shipped, and the fix is a
copy of one function.

## 5. What is actually causing this

Not carelessness. Two mechanisms, both structural:

**Docs assert capabilities; nothing checks the assertion.** `physis-skill` §1
is the right check — *declared ≠ called* — and `physis-check sweep` implements
it for symbols. No check maps a **documented capability** to the code that
would have to exist for it to be true. `transplant` is listed as a capability
with zero call sites, and no gate anywhere would notice.

**Numbers are copied forward, not regenerated.** "194 pass" was true once.
"253 → 165" may still be true. Both are now assertions about the past written
in the present tense, and RESCOPE's preamble promises the opposite.

## 6. The remedy, and it is small

Extend `physis-check` with a `claims` verb, in the shape the skill already uses:

```
physis-check claims [doc ...]
```

For each claim that names a symbol, run the `calls` check on it. For each claim
that quotes a number, require a command in the repository that regenerates it,
and fail when there is none. Claims that name neither are prose and are skipped
— explicitly, and counted, so the proportion of unfalsifiable claims in a
document is itself visible.

That is the existing rule applied one level up, from symbols to capabilities.
It is the same move PH-021 made for the machines: the check exists, it is good,
and nothing runs it where the failures actually happen.

**Do not fix the docs by hand and stop there.** Every item in §4 would have
recurred; two of them are the same error made twice in one day by the same
reader of the same files.

---

## Appendix — commands used

```bash
bash scripts/physis-check.sh sweep src physis-core/src
grep -rn 'transplant' --include='*.rs' src physis-core/src physis-core/{examples,tests} tests
grep -rn 'measure_hybrid_vs_cosine' --include='*.rs' src physis-core/src
grep -roE '"physis_[a-z_]+"' src/mcp.rs | sort -u | wc -l
(cd physis-core && cargo test --lib)
(cd physis-core && cargo clippy --all-targets -- -D warnings)
cargo clippy --all-targets -- -D warnings
for n in 253 165 338 12.11 0.480; do grep -rn "$n" --include='*.rs' --include='*.py' --include='*.sh' \
    src physis-core/src physis-core/examples benchmarks scripts | wc -l; done
```
