---
id: PH-003
track: physis
phase: 0
status: todo
rung: 3
depends: [PH-000]
gate: "bash scripts/tests/licence-consistency.sh"
commit: "branch agent/PH-003; one commit for the decision record, one for the page edits"
---

# Licence claims must agree everywhere (LEGAL, human decision inside)

## Why
pricing.html, about.html, index.html, customer_dashboard.html say the core is
MIT and physis_pro is dual-licensed. physis-core/Cargo.toml declares
Apache-2.0. The root Cargo.toml says physis_pro has no SPDX id because
LICENSE.txt is a proprietary EULA. Any enterprise legal review finds this
before it finds anything else.

## Steps
1. RUNG 5 (human): Gio decides the true statement. Write it as ONE sentence in
   packets/reports/PH-003.md, e.g. "physis-core is Apache-2.0; physis_pro is
   proprietary (EULA in LICENSE.txt)". The agent may DRAFT the options with
   consequences, but MUST NOT pick.
2. After the decision exists in the report: make all four pages agree,
   root Cargo.toml workspace comment agree, and add the licence string to the
   footer template if one exists.
3. Write scripts/tests/licence-consistency.sh: extracts the licence claim from
   each page, asserts all four identical AND equal to a string stored in
   scripts/licence-claim.txt, and that physis-core/Cargo.toml `license` field
   is consistent with it.

## Tests you must add
- The consistency script above IS the test. It must fail if any page is edited
  to disagree (prove it: temporarily edit one page, run, expect non-zero, revert).

## Gate
```bash
bash scripts/tests/licence-consistency.sh   # exits 0
```

## Never
- Do not invent legal text. Do not pick a licence. Do not touch LICENSE.txt.
