# Semantic Navigation — Function 3 (MVP)

> Status: planned — CLI surface below was written from the pre-P0 design of
> `nav` as a `physis-pro` subcommand. As built, `physis-pro` has **no `nav`
> subcommand** (verified: `physis-pro nav --help` → unrecognized subcommand;
> debug binary `physis-pro 0.1.21` of 2026-09-18) while the library seam
> `nav::workspace::NavWorkspace` and the `physis-hud` TUI exist and work
> (`physis-hud --prescan-only fixtures/mvp_propagation` indexed 8 objects).
> Treat the `physis-pro nav …` lines as the P2 target surface, not as a record
> of what runs. P0 exit requires either landing `cmd_nav` or correcting this
> doc. (`physis-hud`, `--features tui`). No LLM anywhere; degraded retrieval is reported, never
> silent. Extension of the relation layer (SAME / DIFFERENT / RELATED,
> provenanced), the graph dual view (`g` key in the HUD), the
> `NavWorkspace` hot-swap seam, and `scripts/physis-update.sh` landed with
> the 2026-09-16 follow-up.

## Hot-swap seam — `nav::workspace::NavWorkspace`

Both frontends (CLI `cmd_nav`, HUD `App`) program against **one façade**:
`src/nav/workspace.rs::NavWorkspace`. It owns the three shared resources
(index, sled store, lazily-selected embedder) and exposes the dozen
operations they actually perform (`update`, `query`, `resolve`, `read`,
pin/relation CRUD, trail, `sem_off`/`sem_on`). The embedder is selected
lazily — loading ONNX weights must not precede the first keystroke, and must
not happen at all for `pins` or `trail`.

Consequence: `nav` internals (including a future split into a separate
crate) can be reorganised without touching either frontend. Free functions
in `nav/mod.rs` stay public for tests and advanced callers.

## Updating without a full reinstall

```sh
just update            # build release + smoke + swap into ~/.local/bin
just update-check      # what is installed vs the current checkout
just rollback          # restore the previous binaries
```

`scripts/physis-update.sh` (also usable directly; `--bin <name>` to select):
builds the requested release binaries, smoke-tests each candidate (licence
gate, panic detection — a TUI binary exiting non-zero without a TTY is not a
failure), then swaps **hard copies** into `~/.local/bin` with a backup in
`target/update-backup/` and a revision stamp in `target/installed-rev`. A
failed build or smoke leaves the installed binaries untouched. Default bins:
`physis-pro`, `physis`, `physis-hud`; features `tui,cli`
(`PHYSIS_UPDATE_FEATURES` overrides).

## What it is

Meaning-first navigation over addressable objects — files, passages, notes —
with hybrid retrieval, per-component score explanation, explicit user
relations, a navigation trail, and immediate Notebook handoff. The filesystem
is one substrate underneath the interface, not the interface itself.

## What it reuses (nothing parallel-built)

| concern | reused from |
|---|---|
| chunking (40-line windows, `path:start-end` provenance) | `pack.rs` |
| embedding cache (content-addressed, atomic save) | `pack::EmbedCache` |
| embedder cascade (ONNX MiniLM, self-test gated, RP fallback labelled) | `embed::select_embedder` |
| cosine | `physis_core::models::cosine_sim` |
| file hashing + sled store | `indexer`, `index.sled` |
| exclusion rules | `scanner::DEFAULT_EXCLUDE_DIRS` |
| Notebook answer path (Function 2) | `ported::notebook::answer` |

## Object model (`src/nav.rs`)

```rust
ObjectId { origin, key }   // origin "fs" · key = content hash, NOT the path
ObjectKind { File, Passage, Directory, Concept, Note, Other }
PhysisObject { id, kind, label, source: Option<SourceRef>, mtime, hash,
               text, embedding }   // embedding serde-defaulted
```

Identity is content-based: a rename does not orphan history or pins. Passages
are first-class; a `path:start-end` region navigates independently of its file.

## Indexing architecture

- **Index file**: `index.json` (one per root, atomic temp+rename write) under
  `PHYSIS_DATA_DIR`. Deliberately **not** in a sled tree — measured: sled kept
  every 22 MB save as a new blob and filled gigabytes of never-reclaimed
  blobs. Small hot records (pins, trail, relations) stay in sled where churn
  is cheap.
- **Incremental pass** (`nav index`): walk metadata only; stat fast path
  (1 ms tolerance); re-hash only moved files; re-chunk + re-embed only changed
  chunks; drop objects of deleted files.
- **Embedding**: shared `EmbedCache` keyed `(embedder_kind, dim, sha256)` —
  an unchanged chunk costs a hash, never a forward pass.

## Hybrid ranking

```
R(q,x) = w_e·semantic + w_l·lexical + w_r·recency + w_p·pin/relation + w_c·context
```

Every component is inspectable per hit (`RetrievalScore.components: BTreeMap`)
and rendered as `why: lexical 1.00 × 0.30 · semantic 0.85 × 0.60 · …`.
Weights live in `NavWeights` (semantic 0.6, lexical 0.3, recency 0.08,
pin 0.25, context 0.15). A leading `/` in the query disables context bias.

Degraded mode: when no embedder passes the semantic probe the header says
`semantic: false (random-projection)` and lexical/structural retrieval carries
on alone. Reduction is reported, never implied.

## Relations (SAME / DIFFERENT / RELATED)

`Relation { a, b, kind, note, origin, status, created_at }` — pins generalise
into this one store:

- `kind`: `"same"`, `"different"`, or any verb (`references`, `supersedes`, …)
- `origin`: `User` · `Deterministic` · `Inferred` — never collapsed; a user
  assertion and an inference must not look identical
- `status`: `Asserted` · `Inferred` · `Disputed`

Effect on navigation: an object rides up when a *related partner* matches the
query (`query_with_relations` merges pins + relations into one scoring path —
two stores, one scorer). No embedding is ever mutated by user intent.

`nav rel-infer` adds deterministic **same-source** relations (shared
provenance) with `origin = Deterministic` — a suggestion never wears
assertion clothes.

## CLI (P2 target — NOT SHIPPED, see status note at top)

```sh
# Each line below is planned `physis-pro nav …`; none of them parses today.
physis-pro nav index   [dir] [--no-embed]     # incremental index + embed
physis-pro nav query   "<query>" [dir]        # /prefix = global (no ctx bias)
physis-pro nav open    <path|id>  [dir]       # exact underlying content
physis-pro nav pin|unpin <a> <b> [--dir D]    # explicit, reversible
physis-pro nav rel     <a> <b> [kind] [--note] [--dir D]
physis-pro nav unrel   <a> <b> [--kind K] [--dir D]
physis-pro nav rels    [target] [--dir D]
physis-pro nav rel-infer [dir]
physis-pro nav parents <target> [--dir D]     # fs / note / kind — plural
physis-pro nav pins | trail | sem-off | sem-on
physis-pro nav notebook "<q>" <paths…> --dir D   # Function 2 handoff
```

## HUD (`physis-hud [root]`, features = ["tui"])

Startup is prescan-first: opening a root runs the incremental index + embedding
pass *before* the TUI takes the screen (first index says so on stderr, later
starts print one summary line), anchors state at `<root>/.physis` like the MVP
loop, and caches the workspace token denominator once — the header never
re-walks the tree per frame. Flags: `--prescan-only` (warm index + caches and
exit, no TUI — scriptable cold-start fix), `--no-prescan` (open the persisted
index as-is, fast restart).

Three panes: grouped neighborhood (Notes / Code & files / Passages / Concepts,
ordered by group size) · reading pane (exact source) · why-this-ranked + query
bar. Kind is encoded by tag (`[file]`, `[passage]`, …) **and** colour; a
legend is always present.

Keys: `/` search · ↑↓ move · Enter zoom in · Esc zoom out (trail preserved) ·
Space select · `p` pin two · `s`/`d`/`R` same/different/relates two · `g` graph view ·
`H` history (git log, else mtime order — same projection as REPL `history`) ·
`B` bench (measured scan/query/propagation numbers — same helper as REPL `bench`) ·
`D` dependents of the cursor path (same rule as REPL `dependencies`) ·
`C` structural pairs over the selection (same helper as REPL `contradictions`) ·
`n` Notebook over selection · `r` related · `q` quit · Ctrl-C anywhere.

Startup shows the REPL-banner facts (files · objects · cached sample map) plus
the absorbed-at-open notice when the workspace moved while closed — both share
the library helpers (`mvp::map_counts`, `mvp::since_last_session_notice`), so the
two surfaces never disagree about the past.

**Graph dual view (`g`)**: the same objects + relations as the list, drawn as
a local ASCII projection around the selected object — one substrate, two
renderings, no second graph model. Edge grammar: `───` user · `═══`
deterministic · `- -` inferred/near. Every relation carries a canonical edge
id (`rel:<a-short>:<kind>:<b-short>`, endpoints ordered — `nav::relation_id`),
shown in the explain pane and the graph legend; the id is derived, never
stored, so no migration exists. Kind→core mapping (`relation_kind_to_core`)
covers only traceable verbs (`extends`→DerivedFrom, `depends_on`→DependsOn,
`causes`→Causes, …); SAME/DIFFERENT/free verbs stay local and say so.

## Semantic parents (plural by design)

`nav parents <object>` shows filesystem parent, note parent (file of a
passage), and kind group. No single fake hierarchy.

## Notebook integration

`nav notebook "<q>" nav.rs rag.rs` reads the canonical objects from disk,
splits whole files into the same 40-line passages, and feeds Function 2's
grounded answer path. Extractive tier with `[n]` citations when no oracle is
configured; the model never owns source selection.

## Storage locations

| record | location |
|---|---|
| index (objects + files) | `PHYSIS_DATA_DIR/nav-index<root>.json` |
| pins | `index.sled` `nav:pins` |
| relations | `index.sled` `nav:relations` |
| trail | `index.sled` `nav:trail` |
| chunk embeddings | `PHYSIS_DATA_DIR/pack_embed_cache.json` (shared with pack) |

## Tests

15 in `src/nav.rs` + `src/nav/workspace.rs` (`cargo test --lib nav::`):
semantic+lexical plumbing, lexical-only degraded mode, pin and relation
bridges, pin/relation idempotence + reversibility, incremental skip, trail
round-trip, exact-line reading, context bias visibility + effect, group
ordering, plural semantic parents, deterministic same-source inference,
workspace end-to-end (update → query → pin → trail → sem-off → sem-on →
read).

## Known failure modes (honest)

- The random-projection embedder is a lexical hash; "semantic" gains only
  exist with the ONNX embedder present. Always labelled.
- Lexical component is term-subset matching, not BM25 — weak rare-term
  weighting; the score slot does not change when BM25 lands.
- Pin/relation bridge boosts one hop only; no transitive multi-hop.

## Deferred to Function 4 / later

Graph ⇄ HUD selection sync beyond the local projection · direct HUD editing
(Function 1) · query operator syntax (`type:`, `path:`, `after:`) · multi-root
config (the `NavRoot` struct exists) · filesystem hard links as a distinct
concept from semantic pins · Command Box action layer · extracting `nav` into
a standalone crate (the `NavWorkspace` seam makes this mechanical when the
feature surface stabilises).
