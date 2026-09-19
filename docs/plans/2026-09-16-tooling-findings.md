# Tooling findings — licence, MCP build, `--features full` (2026-09-16)

Three environment facts that cost time to rediscover. Each was verified by
running the command shown, not by reading a doc.

## 1. The Pro licenser is not lost

Reported as lost; it is present, in three forms, and the dev loop does not need
any of them to change.

| piece | path | what it is |
|---|---|---|
| dev minter | `physis-dev-license/` | a standalone crate (own `[workspace]`, excluded from the root workspace) that prints a licence signed with the **all-zeros dev key**, matching `DEV_FALLBACK_PUBKEY_HEX` in `src/license.rs` |
| vendor tool | `src/bin/license_tool.rs` (`required-features = ["vendor"]`) → `physis-license` | real Ed25519 `keygen` / `sign` / `inspect` / `revoke` / `list-skus` / `sign-artifact` |
| wrapper | `scripts/generate-physis-license.sh` | `keygen [dir]` · `issue --key … --org … --exp …` · `inspect FILE` |

Evidence:

```bash
cargo run --quiet --locked --no-default-features --features vendor --bin physis-license -- --help
# → keygen · sign · inspect · revoke · list-skus · sign-artifact
ls ~/.physis-vendor          # signing.key + verifying.key, 64 bytes each
```

The dev key at `.physis-bench/license.key` still validates:

```bash
./target/release/physis-pro license
# physis: licensed to dev local — Enterprise tier, 1 seats, expires 2099-01-01
```

`DEV_KEY_BUILD` is `true` because the checked-in binaries were built without
`PHYSIS_LICENSE_PUBKEY`, so they accept the all-zeros-key licence. A customer
build stamps the real public key at compile time
(`PHYSIS_LICENSE_PUBKEY=<hex> cargo build --release`) and then only licences
signed by `~/.physis-vendor/signing.key` validate. **Nothing here blocks
development; no action taken.**

To regenerate the dev licence if it is ever deleted:

```bash
cd physis-dev-license && cargo run -q > ../.physis-bench/license.key
```

## 2. The 30-tool Pro MCP server — found broken, now fixed

`src/mcp.rs` — the whole implementation, including all 30 `physis_*` tools — is
inside `#[cfg(feature = "mcp")]`, and `mcp` was **not** in `Cargo.toml`'s
default features. So `.mcp.json`'s `["serve", "--stdio"]` entry could not start
from a plain `cargo build --release`:

```bash
./target/release/physis-pro serve --help
# error: unrecognized subcommand 'serve'
```

`scripts/package.sh` builds `--features full`, which includes `mcp`, so shipped
artifacts were never affected — only the development attachment every agent is
told to use.

**Fixed**: `mcp` is now in the default feature set, and `tests/pro_mcp.rs` is the
gate that would have caught it — it spawns `serve --stdio`, asserts the 30-tool
list (and that the Core workspace tools are *not* in it), and round-trips a write
(`physis_remember` → `physis_recall`, verdict included). The SKU tiers are
untouched: `base` still excludes `mcp`, so a customer build is unaffected.

Three properties of the Pro surface the test pins down, each learned the hard
way:

| property | Core server | Pro server (`rmcp`) |
|---|---|---|
| protocol version | echoes the client's | negotiates **down** to `2024-11-05` |
| reply order | request order | **out of order** (id 4's reply arrived before id 3's) |
| input timing | drains everything written, then exits | drops input written before the handshake completes |

So the client must be sequential and index replies by id — which is what a real
MCP client does, and what `tests/pro_mcp.rs` does. `physis_remember` on the Pro
server also takes `input` + `verdict`, not the Core server's
`text`/`actor`/`outcome`; the two servers share tool *names* and little else.

## 3. `--features full` cannot build on pansa

`full` includes `ocr` → `tesseract` → `leptonica-sys`, which needs the system
Leptonica library:

```
Package 'lept' not found in the pkg-config search path.
The system library `lept` required by crate `leptonica-sys` was not found.
```

This is an environment limit, not a regression. The achievable gates on this box
are the default-feature suite and the targeted `mcp` suite:

```bash
PHYSIS_EMBEDDER=random-projection cargo test                        # 1195 passed
PHYSIS_EMBEDDER=random-projection cargo test --features mcp --test system_mcp
```

Installing `libleptonica-dev` (and `libtesseract-dev`) would make `--features
full` runnable; nobody has needed it yet.

## 4. Another session edits this clone while you work

Not hypothetical: on 2026-09-16 a second session was writing
`src/bin/doc_tui.rs` (untracked) at 11:48:09 and again at 11:49:10, **during** an
equal-budget retrieval run. The pack read a longer revision than the one the
citation check later measured, and the check reported
`bin/doc_tui.rs:1881-1881 (outside file)` — a real gate failure caused by a
moving corpus, not by a packer bug.

Two consequences:

- **Re-check `git status` and file mtimes before any destructive step**, and
  never `git add -A` (FLEET.md says the same; it has already swept another
  agent's work into a commit once).
- **Freeze a corpus before measuring it.**
  `benchmarks/retrieval/equal_budget.py` now copies each corpus root into a temp
  dir (`freeze()`), hashes the copy and packs from the copy, so a concurrent
  writer cannot move the ground under a run. `unresolved citations` is the gate
  that caught it; keep it.

```bash
find src physis-core/src -name '*.rs' -newermt '-5 minutes' -printf '%T+ %p\n'
```

## 5. `cargo fmt --check` is red repo-wide

Pre-existing diffs in `examples/adaptive_doc_demo.rs`,
`src/web/coherence.rs`, `physis-core/src/rag.rs` and elsewhere. `cargo fmt` is
therefore **not** run over the tree — only over files a change rewrote, and
`clippy -D warnings` is the enforced style gate.

**Actor**: continuation session (OpenCode) · **Date**: 2026-09-16
