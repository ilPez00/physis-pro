---
id: PH-ORCH-ROUTER
title: orchestrator delegation failed on a provider quota, not a bad model
status: review
phase: 1
depends: []
claimed_by: opencode
claimed_at: 2026-09-20
---

# PH-ORCH-ROUTER

## Gate

```sh
node --test clients/opencode-orchestration/router.test.mjs   # 23/23
```

## Execution

Files: `.opencode/orchestration.json`,
`clients/opencode-orchestration/router.mjs`,
`clients/opencode-orchestration/router.test.mjs`.
Report: `packets/reports/PH-ORCH-ROUTER.md`.

## Follow-up

Restore `commandcode/*` to the role lists after 2026-09-23 if its window resets,
or keep the live providers and delete the dead entries.
