---
id: PH-CORE-CLEANUP
title: delete dead artifacts named by PLANNING 2c
status: done
phase: 1
depends: [PH-CORE-DOCS]
claimed_by: opencode
claimed_at: 2026-09-20
reviewed_by: opencode
reviewed_at: 2026-09-20
---

# PH-CORE-CLEANUP

## Gate

```sh
cd physis-core
test ! -f src/experiments.rs
test ! -f Cargo.lock.bak
test ! -f config/engineering_ontology.json.bak
test ! -f sw_add_domains.py
test "$(grep -c 'experiments::' ../tests/snapshots/physis_core_api.txt)" = 0
```

## Execution

Branch `agent/PH-CORE-CLEANUP` @ `f3a0e11`. Report:
`packets/reports/PH-CORE-CLEANUP.md`.

## Follow-ups

- Remove the stale `sw_add_domains.py` entry from `Cargo.toml` `exclude`.
- Update `docs/MODULE_MAP.md:37`.
