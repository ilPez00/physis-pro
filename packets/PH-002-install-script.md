---
id: PH-002
track: physis
phase: 0
status: todo
rung: 1
depends: [PH-000]
gate: "bash -n install.sh && ./install.sh --dry-run && ! grep -rn 'target/release/physis-web' --include='*.sh' --include='*.md' . | grep -v packets"
commit: "branch agent/PH-002; one commit for the fix, one for docs"
---

# install.sh is broken as written — fix or delete

## Why
install.sh copies `target/release/physis-web`, a binary this workspace does
not produce (the real one is `physis-pro-web`). It cannot succeed; the first
user who tries it gets a broken install and blames the product.

## Context
- Find every doc referencing install.sh or the wrong binary path.
- scripts/package.sh is the other distribution route — read it first.

## Steps
1. Decide (report, human confirms): FIX install.sh or DELETE it and make
   scripts/package.sh the only documented route. Default: fix it.
2. Add `--dry-run` flag: prints what it would do, touches nothing, exit 0.
3. Make the script idempotent (safe to run twice; overwrite only its own files).
4. Grep docs for the wrong path; correct all of them.

## Tests you must add
- **Integration (shell):** scripts/tests/install.sh.bats OR a plain bash test
  script under scripts/tests/: run `./install.sh --dry-run` in a temp HOME,
  assert exit 0, assert no files created, assert printed plan mentions the
  correct binary name.
- **Unit:** not applicable (shell) — the dry-run test above is the unit.

## Gate
```bash
bash -n install.sh
./install.sh --dry-run
bash scripts/tests/install-test.sh   # the test you wrote
! grep -rn 'target/release/physis-web' --include='*.sh' --include='*.md' . | grep -v packets
```

## Never
- curl|bash patterns that execute remote code; install to system paths without
  asking; delete anything it did not create.
