# E23 — where supervision can come from, and where it cannot

`run_e23.py`. E21's positive rests on 436 human decisions, and its honest limit
was that supervision has to come from somewhere. The proposal under test:
**it need not come from a person.** Compilers, exit codes, test outcomes and
commit metadata label things continuously and for free.

Before building ingestion for any of that, one premise has to hold.

## The premise, and the two cheap tests that shaped it

The axis that needs help is **mode** — what is being *done*.

**Test 1 (E22): is the representation domain-loaded?** Yes. Paired over 40
identical splits, `corpus.json`'s hints help `domain` (+0.022, *t* = +3.13) and
**hurt `mode`** (−0.034, *t* = −5.39, better in 6/40). Subject-matter vocabulary
pulls entries toward what they are about.

**Test 2: does activity-shaped text fix it?** **No.** Entries whose names name
an activity ("Abductive Reasoning", "Agent Coordination") are **not** better
predicted on mode than the rest: **−0.009, null**. So the tempting fix —
"describe entries in activity language" — is not supported. *(Crude
operationalisation: nominalising suffixes on the head noun. A null on a crude
proxy is weak evidence, and is reported as weak.)*

That leaves the stronger reading, still a hypothesis: **mode may not be
recoverable from any static description of an entry, only from observing it in
use.** Which is exactly what an execution trace is.

## The test: can activity text carry an activity label?

Data where the label **is** an activity and the text **is** about that activity:
528 conventional-commit subjects from this repo, 6 types with n ≥ 14. The type
prefix is the label, so it is **stripped before embedding**; a TF-IDF arm runs
alongside to separate activity semantics from lexical convention.

| scorer | top-1 | top-2 |
|---|---:|---:|
| **TF-IDF (lexical)** | **0.628 ± 0.026** | 0.803 |
| embedding (`nomic-embed-text`) | 0.575 ± 0.037 | 0.822 |
| permuted (null) | 0.222 ± 0.059 | 0.452 |
| majority floor | 0.451 | — |

**The reading was fixed before the run:** *if TF-IDF matches the embedder, the
signal is lexical convention rather than activity semantics, and does not
transfer to entries carrying no such convention.*

**TF-IDF does not merely match it — it beats it.** So the label is recoverable
because it was *written in conventional words by the same author*, not because
the text encodes what was done. A "fix" commit says crash, regression, guard.

## What this settles

**Machine supervision is real where a convention or an exact log carries the
label, and is not a route to inferring causes that were never written down.**

That is the empirical backing for the rule now in `PLAN.md`:

> **Causes are user-assessed unless an exact error log is provided.** Where the
> signal is explicit — a compiler error, an exit code, a stack trace — take it
> literally. Where it is not, hand it to the person and do not infer.

An exact error log needs no semantics: it *is* a structured label, and reading it
literally is not the failed inference tested above. The failure mode this rules
out is the tempting one — treating circumstantial execution signal as evidence
about a concept's cause or category.

## The two traps for anyone building this anyway

1. **Frequency.** Execution signals are dominated by how often things are run;
   the commonest errors come from the commonest commands. Supervision derived
   from counts risks re-encoding frequency — the confound that killed ANCHORVOC
   across E17/E18, where entry length alone scored 0.619. **A frequency-only
   control built from the same traces is mandatory, not optional.**
2. **The code→concept leap.** A borrow-check error is evidence about a program,
   not about whether "Error Handling" belongs in HEAL/MAINTAIN. The tie has to be
   causal, and asserting it is exactly the unvalidated step this track has
   punished seven times.

## Status of the data

`vault.rs` (Obsidian, git log), `history.rs` (browser, OPML, chat JSONL) and
`praxis.rs` (backfill) are shipped importers, but **no activity corpus is
materialised** — `~/.physis-core/nodes.json` is 24k. Building that ingestion is
the cost of pursuing this, and on the evidence above it should be spent on
sources that carry explicit labels rather than on ones needing interpretation.
