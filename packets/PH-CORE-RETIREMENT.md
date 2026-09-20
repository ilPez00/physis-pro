---
id: PH-CORE-RETIREMENT
title: reconcile RETIRED.md with an actively committed submodule
status: review
phase: 1
depends: [PH-CORE-CLASSIFY]
claimed_by: opencode
claimed_at: 2026-09-20
---

# PH-CORE-RETIREMENT

Recommendation only; no code or doc change. **Human decision required.**

## The question

`physis-core/RETIRED.md` says the repository is retired historical, "no longer
the active development repository", while `origin/master` received the split
commit and a grid rework after the retirement commit, and `physis_pro`
path-depends on the checkout.

## Evidence

See `packets/reports/PH-CORE-RETIREMENT.md`. Headlines:

- retirement docs were added by `9426b91` (2026-09-19), the same commit that
  removed the product modules — not at `d65858b` (2026-09-14) as `RETIRED.md`
  claims;
- three commits landed on `origin/master` after `9426b91`;
- no tag after `v0.1.24`, `Cargo.toml` is `0.1.26`, `CHANGELOG` says unreleased;
- local `master` (`3a7321a`) and `origin/master` (`ed8dc29`) have diverged.

## Recommendation

Option B — correct the retirement docs to match practice: freeze the **public
Apache-2.0 release line** at `v0.1.24`; state that the branch continues as the
engine `physis-pro` vendors; correct the "last historical commit" claim. If the
intent is genuinely retirement, then stop committing to this repository and move
the engine to its successor.

## Gate

Not executable — a human decision. The packet is complete when
`RETIRED.md`/`docs/REPOSITORY_RETIREMENT.md` and practice agree, or the engine
has moved to a successor repository.
