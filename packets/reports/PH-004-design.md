# PH-004 — Design: a semantic self-test that can FAIL

Rung 3. Written before code, per the packet. This freezes the acceptance
contract; the implementation step must satisfy exactly this and nothing more.

## 1. The failure it closes

The old probe (`"a photograph of a dog"` vs `"a photograph of a puppy"`) shares
4/5 tokens, so a bag-of-words hash passes. The random-projection embedder
passed; `/health` reported `"semantic": true, "degraded": false` while the CLI
warned the same embedder was non-semantic (PLAN.md H2). A guard that cannot
fail is not a guard.

## 2. The discriminant (already in the code, and why it is the right shape)

`src/embed/mod.rs` already implements the H2 fix: three lexically DISJOINT
pairs (car/automobile, dog/puppy, spindle-overheat/thermal-fault), each required
to beat its distractor by `MARGIN = 0.05`. The disjointness is itself a test
(`probe_pairs_are_lexically_disjoint`), and random projection is pinned to fail
(`random_projection_fails_the_semantic_probe`). This design endorses that shape;
step 3 (below) adds only what is observably absent:

## 3. What is missing (three holes, each with evidence)

1. **No CLI `selftest`.** The packet gate demands
   `cargo run --bin physis-pro -- selftest` exit 0. `src/main.rs` has
   `Commands::Doctor` but no selftest subcommand — the gate cannot pass as
   written. Required: `Commands::Selftest` that runs the SAME
   `semantic_self_test` over the cascade-resolved embedder and exits 0 on pass,
   1 on fail, with a per-probe verdict and a fixed ≤2 s budget on the default
   build (3 tiny embeds × 2 channels; no model load on the RP path).
2. **`physis-core` still trusts the lax probe.** `physis-core/src/embed.rs`
   asserts `semantic_self_test(&RandomProjectionEmbedder::new(384))` PASSES,
   which contradicts the monolith's "RP must fail" pin. The fix must not change
   the core embedding space (Never); it must add the disjoint regression in
   core: a fake overlap embedder that passes the old sentence pair but fails
   the disjoint synonym pairs — proving the old sentence probe alone is not the
   criterion.
3. **The MiniLM flat-fallback half of the packet test is conditional.**
   Weights live at `models/model.onnx` here, so the "passes with real weights"
   arm can actually run; the "skipped if weights absent" arm must assert the
   skip is visible (stderr marker), never silent.

## 4. Acceptance (mirrors the packet gate plus its Tests)

- `cargo clippy --all-targets -- -D warnings` clean.
- `cargo test semantic_self_test` — the filter must mean something: the new
  CLI-subcommand test and the disjoint-probe regression carry
  `semantic_self_test` in their names; the existing RP-must-fail
  unit test already does.
- `cargo run --bin physis-pro -- selftest` exits 0 with RP forced
  (`PHYSIS_EMBEDDER=random-projection` ⇒ expected pass of the *instrument* on
  the wrong embedder = the command reports fail verdict, exit 1) — see §5.
- /health contract (pinned by `health_and_embedder_are_public`, unchanged):
  explicit `random-projection` ⇒ `semantic=false, degraded=false`.

## 5. Exit-code semantics for `selftest` (decided here, so the gate is checkable)

`selftest` reports on the OPERATOR-SELECTED embedder, not on a fixed one:
- embeds via `select_embedder` (same cascade the server and CLI use: override
  → model candidates, each self-test gated → RP last resort);
- prints one line per probe triple plus the two held facts
  (`semantic`, `degraded` as the server derives them);
- exits 0 iff the selected embedder passes `semantic_self_test`;
- therefore with RP forced it exits 1 (the instrument working), and the packet
  gate line `... selftest 2>/dev/null; test $? -eq 0` passes only under a
  semantic embedder. The design makes the ambiguity explicit instead of
  burying it in `2>/dev/null`.

## 6. Non-goals (from Never)

- No network, no model downloads at test time.
- No embedding-space change: RP stays RP; ONNX candidates untouched; the only
  core change is a TEST fake plus a regression test, both offline CPU.
- No new cache keys: the probe does not change, so `SEMANTIC_PROBE_VERSION`
  and `SelfTestCache` stay as-is.
