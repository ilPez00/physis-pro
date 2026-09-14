# E81 — ingestion pipeline, and what static rule dictionaries can actually do

Two things were asked for: ingestion pipelines over heterogeneous content, and
static rules (dictionaries) for the symbolic layer. Both are built; the second
one failed its first measurement, and the failure localised precisely.

## The pipeline

`benchmarks/conatus/ingest.py` walks a root and dispatches per media type:

| media | extractor | on this machine |
|---|---|---|
| text / code | direct read | works |
| pdf | `pdftotext -layout` | works |
| video, audio, images | transcript / OCR / probe | **absent** — `ffprobe`, `tesseract`, `whisper`, `exiftool` are all missing |
| anything else | — | skipped, counted, with the reason |

Detected extractors are printed at start-up, because **a pipeline that silently
skips every PDF and one that has no PDFs produce the same empty result.** Media
files are not dropped: they are ingested with metadata and
`content_unavailable: no transcriber or OCR installed`, so the gap is a row
rather than a silence.

On `~/dev/physis_wow`: 85 files seen, 85 ingested, 51 with text — 49 text/code,
2 PDFs extracted, 24 media held as metadata, 10 with no extractor.

## The static rules, and the null that caught them

`relations.json` maps surface cues onto the relation vocabulary that already
exists in `physis-core/src/relation.rs` (`Causes`, `Explains`, `Precedes`,
`DependsOn`, `Produces`, `Measures`, `Observes`, `Contradicts`, `Supports`,
`Invalidates`, `Predicts` — and `Causes` and `Explains` are separate types, so
the cause/reason distinction is in the vocabulary before anything tests it).

The null: run the same extractor over the **same words in shuffled order**
inside each sentence. A rule set that fires as often on scrambled text is
detecting vocabulary, not structure.

**Version 1 — every cue a single word or loose phrase, arguments taken as
everything before and after:**

```
physis_wow    299 relations · shuffled null 310 · ratio 0.96
docs/        1510 relations · shuffled null 1357 · ratio 1.11
```

The aggregate said *no structure*. Tightening the arguments to short contiguous
spans that must carry content words changed nothing (0.96). But decomposing the
null by cue arity localised it exactly:

```
multi-word cues : real 15 · null  1 · ratio 15.00
single-word cues: real 284 · null 309 · ratio 0.92
```

**The dictionary was two instruments mixed together.** "depends on" cannot
survive a shuffle; "requires" always can. The aggregate ratio of 0.96 averaged a
working rule set with a broken one and reported the mean.

**Version 2 — every cue anchored to the function word that must follow it in
running text** (`requires (the|a|an|its|two|three)`, `because (the|a|an|it|of)`,
`leads to (the|a|an)?`). No parser, no tagger — just the observation that a verb
and its determiner are adjacent in real text and separated by a shuffle:

| corpus | relations | shuffled null | ratio |
|---|---:|---:|---|
| `physis_wow` | 25 | 7 | **3.57** |
| `docs/` (103 files) | 195 | 47 | **4.15** |

Most order-sensitive cues on `docs/`: `depends on` 8:1, `reports the` 8:1,
`before it` 8:1, `because the` 14:2, `needs a` 13:2, `before any` 6:0.

Yield falls from 1510 to 195 on the same corpus. That is the honest trade:
**v1 extracted six times more relations and none of them were evidence.**

## What this says about the approach

Static dictionaries **do** work for symbolic linkage, under one constraint that
the measurement, not intuition, produced: **a cue must span at least two tokens
whose adjacency is grammatical.** That is enough to be order-sensitive without a
parser, which is what makes the approach cheap enough to run over every ingested
file.

It is not enough for argument extraction. The subject and object here are
`ARG_WINDOW`-token spans around the cue, and nothing checks that they are
constituents. Their quality is **untested** — the ratio above measures whether
the *relation fired*, not whether the *arguments are right*, and the next
measurement has to separate those.

Distribution on `docs/`: `DependsOn` 38, `Explains` 35, `Produces` 32, `Follows`
30, `Precedes` 25, `Measures` 14, then single digits, with `Causes` at **1**.
Causal language is nearly absent from this repository's prose while explanatory
language is abundant — 35 `Explains` against 1 `Causes`. Whether that is the
corpus being careful about causation or the dictionary's causal cues being too
narrow is **not decided by this experiment**.

## Limits

One machine's tooling: no OCR, no transcription, so video and audio contribute
only metadata and every claim here is about text and PDFs. The shuffle null
destroys word order within a sentence but preserves sentence boundaries and
vocabulary, so it tests adjacency and nothing else. Argument spans are unvalidated.
`docs/` is one author's prose, which is the same limit every experiment in this
series has.

## Reproduction

```sh
python3 benchmarks/conatus/ingest.py ~/dev/physis_wow
python3 benchmarks/conatus/ingest.py docs \
    --dictionary benchmarks/conatus/relations-anchored.json --limit 300
```
