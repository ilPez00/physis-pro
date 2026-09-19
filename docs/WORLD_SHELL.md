# `physis-world` — the navigational layer, running on this machine

> Historical implementation/experiment report (2026-09-14). The standalone
> `physis-world` command below is not a current Pro binary target.
> Current navigation: [Semantic Navigation](SEMANTIC_NAVIGATION.md).
> Current sequencing: [impossible computer programme](plans/2026-09-19-impossible-computer.md).
> Measurements below remain attached to their original implementation.

Built 2026-09-14, on top of the six experiments E55–E60. It is the smallest
thing that makes the intended transition visible:

```text
FILES → observations → representations → structure → world states
      → position in the log → navigation
```

**The filesystem is not replaced.** `observe` appends to a log and writes
nothing else; every file stays exactly where it is. The world model is a second
layer that can be deleted without losing anything.

## Quickstart

```bash
export PHYSIS_MODEL_DIR=$PWD/models/bge-base-en-v1.5 PHYSIS_MODEL_DIM=768
cargo build --release --features embed-onnx --bin physis-world

target/release/physis-world observe --root ~/dev --max 1500
target/release/physis-world inspect
target/release/physis-world locate worldstate
target/release/physis-world history worldstate
target/release/physis-world why 1650
target/release/physis-world retrieve "which experiment measured the null for the grid"
```

Without `PHYSIS_MODEL_DIR` it falls back to the lexical hash and says so on
stderr. Every answer names the representation it used.

## What it did on the first real run

3232 observations (2580 filesystem, 400 git, 252 terminal) over `~/dev`:

```text
3020 positions carry at least one extracted link (93%)
3805 mutual-kNN edges, 919 components
derived transitions: emerge=2767 none=212 recur=128 persist=119 disappear=6
```

`retrieve "which experiment measured the null for the grid"` returns the E21,
E19 and E19b research commits — the three places in four years of history where
that null was actually built.

## The verbs

| verb | what it does | what backs it |
|---|---|---|
| `observe --root D [--max N]` | append what is there now | `observe.rs` watchers (fs, git, shell) |
| `inspect` | what the world contains | components + derived transitions |
| `locate T` | where a thing sits | entity links, with string-match fallback |
| `neighbors T` | what is structurally near it | mutual-kNN, k=5 |
| `history T` | every position mentioning it, in log order | position = the temporal coordinate |
| `changed T` | positions whose derived transition is not persistence | E56's set operations |
| `compare A B` | shared links, shared neighbours, similarity | — |
| `trace P` | the chain behind one position | observation → representation → operator |
| `why P` | what a position follows from | **both** arms, side by side |
| `retrieve Q...` | nearest positions, then navigate from the best one | E57/E59/E60's arm |

## Three rules the interface obeys

1. **Every answer carries its chain.** `observation seq → representation →
   operator → position`. A navigational layer that cannot say why it said
   something is an oracle.
2. **"No link" is never printed as "not here."** When the structural layer
   misses, the shell falls back to string match and labels the fallback. The
   link layer's recall is the measured bottleneck (E60), and hiding that would
   reintroduce Gap 3 — a retriever with no way to report absence.
3. **`why` shows the linked arm and the blind arm together and does not
   choose.** Which one wins is corpus-dependent and measured: linked 0.700 vs
   blind 0.013 on generated text (E57), linked 0.209 vs blind 0.349 on this
   repository's own prose (E60).

## Two defects found by using it, both fixed here

- **`watch_git` re-appended every commit on the second `observe`**, because it
  takes no `known` list. Visible as `why 1650` answering with position 570 at
  cosine 1.000 — the same commit twice. Fixed at the writer by dropping
  already-recorded `(source, subject)` pairs, and at the reader by collapsing
  duplicates for ranking. **The log itself is not rewritten**: it is append-only,
  and deleting history to tidy a display is the move this architecture exists to
  refuse.
- **`observe::append` returns `(first_seq, last_seq)`, not a count.** The first
  version printed `1733 new, 1732 in log`, which is an off-by-a-whole-meaning
  that a reader trusts precisely because the tool printed it.

## What this is not

It is not an operating system, and it does not claim the model understands
anything. It is a measured retrieval arm plus a derivation over log order,
wrapped in verbs, with the numbers that back each one printed where they can be
checked. The honest summary of the series behind it:

- geometry alone does not transfer across representations (E55, at the null);
- order carries recoverable structure (E56, z = +4.88 vs shuffled);
- structure + order + geometry answer supersession where each leg alone is ~0
  (E57, 0.700 vs 0.013);
- the link layer is extractable without a model where entities are named
  specifically (E58, E59) and not where they are not (E60), and a bad link layer
  is worse than none.
