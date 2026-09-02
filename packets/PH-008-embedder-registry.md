---
id: PH-008
track: physis
phase: 1
status: todo
rung: 2
depends: [PH-004]
gate: "cargo clippy --all-targets -- -D warnings && cargo test --test http_surface image"
commit: "branch agent/PH-008"
---

# Images/video: web server must use the registry's image slot

## Why
The web server calls `embed_image()` on the single active embedder rather than
the registry's image slot. With bge-base active, images simply do not work —
and nothing says so honestly.

## Context
- bge-base has NO image capability. jina-clip-v2 (which does) must NEVER be
  downloaded into models/ (it would hijack the text slot — trap in mesh mem).
  This packet must work WITHOUT jina-clip-v2 on disk.

## Steps
1. Find the registry's image-slot selection logic; find the web path that
   bypasses it. Fix the bypass.
2. Honest degradation: when the active image embedder cannot handle images,
   /api image endpoints return a structured error (503 + machine-readable
   reason), never silent nonsense.
3. /health gains `image_embedder: {kind, degraded}` mirroring the text slot.

## Tests you must add
- **Unit:** registry returns the image slot embedder; a text-only registry
  yields an explicit error kind.
- **Integration:** POST an image with bge-base-only registry → 503 + reason
  field; /health shows image_embedder.degraded=true; text endpoints unaffected.

## Gate
```bash
cargo clippy --all-targets -- -D warnings
cargo test --test http_surface image
cargo test embed
```

## Never
- Download jina-clip-v2. Change the text embedder. Store image embeddings with
  a different embedder than later queries will use (embedder consistency rule).
