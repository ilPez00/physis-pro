---
id: PH-028
track: physis-pro
phase: 0
status: review
claimed_by: kiro
claimed_at: "2026-09-21T09:42:00+02:00"
rung: 3
depends: [PH-026]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --lib nav:: && cargo test --features tui --bin physis-hud && physis-hud <fixture> --snapshot=100x30 --view=world  # plus the offline arm: import with PHYSIS_REMOTE unset must index and navigate with no network"
commit: "branch agent/PH-028; conventional commits; per PROTOCOL §6"
---

# Prepopulating the world: ship it, don't rebuild it

The client (`feather`, `docs/FEATHER.md`) has no embedding model, 6 cores and 7 GB
of RAM, so its first index is the slowest possible way to get a world. This
packet is the design research for the three ways to hand it one, with the
measurements that decide between them. **Nothing here is built yet**; the gate
line above is what a build of option A or B must satisfy.

## What the measurements say (2026-09-21)

| measured | value | why it matters |
|---|---|---|
| whole pansa store `.physis/` | **67 MB** | the world is small enough to ship, and `~/.physis` on the client is one directory |
| of that, `pack_embed_cache.json` | **34 MB** | vectors dominate; everything else is structure |
| `index.sled` | **528 KB** | relations, pins, trail — the parts that are *not* files |
| per-root index file | `nav-index<canonical-root-with-underscores>.json` (`src/nav/mod.rs::index_path`) | the file name is the root path, so a shipped index needs the client's root to be named the same, or a re-tag on import |
| `PhysisObject.source.path` | **root-relative** (`"path": "README.md"`) | an index is portable to a clone whose *relative* layout matches; exact-source reading then needs the file present locally |
| object identity | content hash in `ObjectId.key` | survives the move; a client clone of the same commit has the same ids |
| `PhysisObject.embedding` | **no embedder recorded** | shipping vectors without a stamp is a silent space mismatch |
| `pack_embed_cache.json` keys | `bge-small:384:<content-hash>` | the shared cache *already* stamps kind + dim — the pattern the index needs |
| pansa has no repo-root index | `nav-index_home_gio_physis_pro.json` **absent** | step zero for every option: build the world once, on pansa |
| `GET /api/v1/embedder` from feather | **200 in 10 ms** over the tailnet (`100.65.59.18:19876`) | a served API is reachable where tailscale *SSH* is not |
| `POST /api/v1/search/nodes`, `/api/v1/quality` | **401** without a token | the useful endpoints need a token; the client must hold one |
| `physis-pro-web` binding | `0.0.0.0:19876` | fleet rule says bind the tailnet IP; a partial door (probe open, data closed) |

## Option A — ship the store (cheapest, do this first)

Copy `.physis` to the client and re-tag the index file to the client's root, or
give `NavWorkspace` a `--root-alias` so the tag is explicit rather than a rename
nobody documents.

* Cost: 67 MB once, ~0 on the client; the world is complete and offline.
* Works because identity is the content hash and paths are root-relative.
* Fails when the client's clone is at a different commit: ids that exist on one
  side and not the other are simply absent — honest, and visible in the counts.
* Does **not** survive an embedder mismatch (see the stamp below).

## Option B — export/import a portable corpus bundle (the durable form)

`physis-pro nav export --root R --out bundle.json` on pansa;
`physis-pro nav import bundle.json` on the client, merging into its local index.

* The bundle carries objects, relations, pins and file records — in the existing
  shapes, no new store.
* **It must carry the embedder stamp** (`kind:dim:count`), because
  `PhysisObject.embedding` does not name its producer while the embed cache key
  does (`bge-small:384:<hash>`). The importer refuses a mismatched space, or
  re-embeds, but never silently compares vectors from two models — the failure
  `packets/README.md` already guards against for
  `models/jina-clip-v2`.
* Cheaper to test than A (one pure function pair), and it survives store-format
  changes that raw copying does not.

## Option C — pansa as a server (the online leg, never the foundation)

Measured feasible: the tailnet reaches `physis-pro-web` in 10 ms, and the probe
endpoint is open. Two distinct uses, and they must not be conflated:

1. **Vector/classify offload** — the client keeps a local index but asks pansa's
   `bge-base` to embed new content while reachable, caching by content hash, so
   everything embedded once stays usable offline forever. This is the best value
   per byte: it fixes the client's actual deficiency (no model) rather than
   shuttling structure.
2. **Remote retrieval** — the client asks pansa to answer queries. Online-only,
   so it is a labelled mode (`remote: pansa` in the header, `degraded` when
   unreachable), never the default, and it must degrade to the local index with
   the label still showing.

Requirements either way: a token (env-var **name** only, never printed, per the
repo's secret rule), `PHYSIS_HOST=<tailnet-ip>` instead of `0.0.0.0`, and a
latency budget — 10 ms is the probe, not a query; measure the real call before
claiming an interactive mode.

## Order

1. Build the world on pansa (no repo-root index exists yet) and measure the
   first-index cost there.
2. Option A, with the re-tag documented, to get the client usable today.
3. Option B once a second consumer needs the world (a second client, or a
   different store format), with the embedder stamp and its refusal test.
4. Option C only for embeddings, and only with the label — the offline contract
   in `docs/plans/2026-09-20-physis-hud.md` is the constraint that decides this,
   not the convenience of a fast box.

## Never

- Never a silent remote fallback: an unlabelled network leg turns an offline
  machine into a broken one.
- Never compare vectors from two embedders; ship the stamp or re-embed.
- Never expose the token, and never bind the API to `0.0.0.0`.
