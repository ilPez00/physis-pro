---
id: PH-010
track: physis
phase: 2
status: todo
rung: 2
depends: [PH-001, PH-003, PH-007]
gate: "bash scripts/release-check.sh && git tag --list 'v*' | wc -l"
commit: "branch agent/PH-010; script commit, then first tagged release commit"
---

# Release engineering: tags, changelog, release-check script

## Why
Version drift (PH-001) happened because there is no release ritual. A product
with an enterprise story needs: every merge to main = releasable state,
versions tagged, changelog a human can hand to a customer.

## Steps
1. scripts/release-check.sh: asserts (a) clippy+test green, (b)
   CARGO_PKG_VERSION > last git tag, (c) CHANGELOG.md has an entry for this
   version, (d) licence consistency script (PH-003) passes, (e) bench-pack
   smoke passes. Exit non-zero with a printed reason for each failure
   (silent gates make models hallucinate).
2. Adopt Keep-a-Changelog in CHANGELOG.md; backfill from git log (rung 1,
   mechanical, verify against diff stats).
3. Tag the current state v0.2.0 (or whatever version is honest) as the first
   tagged release — WITH the human's go-ahead recorded in the report.
4. Document the ritual at the top of CHANGELOG.md in five lines.

## Tests you must add
- **Shell tests** for release-check.sh: forged stale-version case, missing
  changelog case — each must fail with the right reason string.

## Gate
```bash
bash scripts/release-check.sh
git tag --list 'v*' | tail -1   # exists
```

## Never
- Tag without the human go-ahead recorded. Re-tag/move a tag (that is a
  history rewrite).
