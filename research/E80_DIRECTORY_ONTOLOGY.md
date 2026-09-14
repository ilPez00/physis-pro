# E80 — does context matching per directory recover what a project *is*?

The proposal: a directory holding two android apps, two webapps, some PDFs and
videos should sort itself out by content — an ontology induced per directory,
recursively — so search and fast diffs work across siblings with no schema
declared anywhere.

Falsifiable at the level that matters: if content embeddings recover the project
kind, the induced grouping is doing work; if not, the directory listing is doing
the work and the embedding is decoration.

**Ground truth never touches the text.** Labels come from build signatures:
`build.gradle*` → android, `Cargo.toml` → rust, `package.json` without gradle →
webapp, a `*.py` majority with none of those → python. Assigned before any
embedding runs.

10 projects in `~/dev`, 4 kinds, 14 sampled files each (140 files), first 3000
characters per file, `onnx-explicit` / bge-base-en-v1.5 / dim 768.

## Result

| arm | accuracy | null (label-permuted) | |
|---|---:|---|---|
| **file level**, nearest neighbour | **0.643** (140 files) | [0.207, 0.371] | **beats the null** |
| project level, **pooled** vector | 0.400 (4/10) | [0.000, 0.500] | inside the null |
| project level, **bottom-up vote** | 0.600 (6/10) | [0.000, 0.500] | just above the band, n = 10 |

**1. Content sorts a directory — at file granularity.** 0.643 against a
permuted null topping out at 0.371 is decisive on 140 files. A file carries what
kind of thing it belongs to.

**2. Pooling a project into one vector destroys that.** 0.400, inside the null,
and the misses say why: `praxis-android → praxis_webapp`, `repowise →
multiagent`. Averaged over its files, every project converges on "a software
project". This is E79's finding again, one level up: a coarse vector is only
weakly a summary of its parts.

**3. The recursion has to run bottom-up.** Letting each file vote with its
nearest cross-project neighbour and taking the directory's majority gives 0.600
against the pooled 0.400 — better, and still only just past the upper edge of a
wide null, because ten projects is a small sample. Reported as **underpowered,
not established**.

## The ground truth is itself leaky, and that is the interesting part

`praxis_webapp` was voted *android* with **0% of its files voting webapp**;
`trustgraph` and `repowise` voted *python*. Looking at why: `praxis_webapp`
contains an android client subtree, `repowise` is 63 Python files with a
`package.json` at the top. The signature label describes the repository's root;
the files describe themselves.

So the label "this project is a webapp" is wrong at file granularity, and the
per-directory ontology the proposal asks for is exactly the repair: induce the
kind of each *subtree* rather than of the repository, and let a directory hold
several kinds without contradiction. The measurement failure and the design
proposal point the same way.

## The symbolic layer that would come next

Reconnaissance, not experiment:

* `physis-core/src/relation.rs` already carries a typed relation vocabulary —
  `Supports, Contradicts, Causes, Influences, Precedes, Follows, DependsOn,
  Requires, Produces, Measures, Observes, Explains, Predicts, Invalidates,
  DerivedFrom` — and is live (4 call sites in core, 7 in Pro). Note that
  `Causes` and `Explains` are already distinct types: the cause/reason
  separation exists in the vocabulary before anything tests whether it is
  recoverable.
* `physis-core/src/linkage.rs` ("which `(domain, mode)` cells does real data
  bridge") has **zero call sites** outside its own demo example. Another
  declared-never-called, and it is the module whose name matches the symbolic
  linkage step.

What is missing for subject–object–verb relations over words rather than
content is an extractor: nothing in either crate turns text into typed triples.
That is the next build, and it should be measured the same way as here — a
relation set with a permuted null, not a demo.

## Limits

Ten projects, one machine, one person's code. Files are sampled at 14 per
project and truncated at 3000 characters. The null band at project level spans
[0.000, 0.500] because there are ten items; nothing at that level is
established, only ordered. Videos and PDFs, named in the proposal, are not
tested at all — every file here is source or markdown.
