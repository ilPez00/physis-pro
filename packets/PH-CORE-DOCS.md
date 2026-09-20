---
id: PH-CORE-DOCS
title: physis-core documentation drift sweep after the Core/Pro split
status: done
phase: 1
depends: []
claimed_by: opencode
claimed_at: 2026-09-20
reviewed_by: opencode
reviewed_at: 2026-09-20
---

# PH-CORE-DOCS — physis-core documentation drift sweep

## Context

The Core/Pro split relocated product modules out of `physis-core` into
`physis-pro` (`physis-core/MOVED_TO_PRODUCT.md`). `physis-core/README.md`,
`CHANGELOG.md` and `.physis-capabilities` still advertised those modules,
features and CLI verbs.

## Gate

Docs only. Acceptance is textual:

```sh
cd physis-core
test "$(grep -cE 'physis-core (studio|notebook)' README.md)" = 0
test "$(grep -cE 'physis_core::(studio|vault|history|praxis|quality|notebook)' README.md)" = 0
test "$(grep -cE '^\| `studio`' README.md)" = 0
! grep -qE 'PhysisConfig|physis-core studio|PHYSIS_STUDIO_HOST' README.md CHANGELOG.md .physis-capabilities
! grep -qE 'physis-system-tui|physis-world|physis system' CHANGELOG.md
```

## Execution

See `packets/reports/PH-CORE-DOCS.md`. Branch `agent/PH-CORE-DOCS` @ `4d18293`
in the `physis-core` submodule.

## Follow-ups (new packets)

1. Doctest/compile gate for README snippets.
2. Source-side stale comment `src/store.rs:26` and the grid-size prose in the
   Core-vs-Pro table.
