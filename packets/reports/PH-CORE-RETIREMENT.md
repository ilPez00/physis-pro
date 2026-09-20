# PH-CORE-RETIREMENT — is `physis-core` retired or active?

**Status:** recommendation only, no code or doc change. Human decision required.

## The contradiction

`physis-core/RETIRED.md` and `physis-core/docs/REPOSITORY_RETIREMENT.md` state
the repository is a retired historical repository, "no longer the active
development repository" (retirement date 2026-09-14). But the checkout carries
unreleased 0.1.26 work, `physis_pro` path-depends on it, and it received commits
after the retirement commit.

## Evidence (native, gathered this session)

```
git log --format='%h %ad %s' --date=short -8
  30cde1a 2026-09-20 fix(classify): ClassifiedSentence.text carries the input text
  f3a0e11 2026-09-20 chore(core): delete dead artifacts named by PLANNING 2c
  4d18293 2026-09-20 docs(core): drop claims for modules retired by the Core/Pro split
  ed8dc29 2026-09-20 fix(grid): migrate 35 ontology packs to the nine-mode axis
  9359b46 2026-09-19 refactor(grid): rework the mode axis to nine owned modes
  c3ddb8e 2026-09-19 style: apply cargo fmt across the workspace member
  9426b91 2026-09-19 refactor(core): retire the product modules, keep the austere core
  d65858b 2026-09-14 feat(system): delegation planning, an MCP surface for it, ...

git log --oneline -- RETIRED.md docs/REPOSITORY_RETIREMENT.md
  9426b91 refactor(core): retire the product modules, keep the austere core

git log --oneline -3 origin/master
  ed8dc29 fix(grid): migrate 35 ontology packs to the nine-mode axis
  9359b46 refactor(grid): rework the mode axis to nine owned modes
  c3ddb8e style: apply cargo fmt across the workspace member
```

- The retirement documents were **added by `9426b91`** (2026-09-19), the same
  commit that removed the product modules — not at `d65858b` (2026-09-14), which
  `RETIRED.md` names as "HEAD at audit time".
- **Three commits landed on `origin/master` after the retirement commit**
  (`c3ddb8e`, `9359b46`, `ed8dc29`), and this session added three more on local
  branches. A retired repository does not receive grid reworks.
- `origin/master` is at `ed8dc29`; the parent `physis_pro/Cargo.toml:57`
  path-depends on this submodule (`version = "0.1.17"`, actual `0.1.26`).
- Local `master` (`3a7321a`) is a **different, diverged history** that does not
  contain `RETIRED.md`; only `origin/master` carries the retirement docs.
- No tag exists after `v0.1.24`; `Cargo.toml` is `0.1.26` and `CHANGELOG.md`
  says "unreleased". So the *release* line did stop at `v0.1.24`, even though
  the *branch* did not.

## The two readings

**Option A — retirement applies to the public release line only.** The repo is
the vendored engine for `physis-pro`; "retired" means no further Apache-2.0
releases, and the future public Core is a separate repository
(`docs/SPLIT_PLAN.md:28-39`, "FUTURE PUBLIC: `physis-core/`"). Under this
reading `RETIRED.md` is directionally right but misleading: it says "no longer
the active development repository" while being the only place the engine is
developed.

**Option B — the retirement docs are stale and should be corrected.** Work
continued on `origin/master` after retirement (`9426b91` → `ed8dc29`), so the
document no longer describes the branch. Fix the document to match practice:
freeze the *public release* at `v0.1.24`, state that the branch continues as the
engine `physis-pro` vendors, and keep the Apache-2.0 grant history intact.

## Recommendation

**Option B**, because it matches observed practice and removes the trap for the
next agent: `RETIRED.md` currently tells a reader the checkout is frozen history
while `physis-pro` compiles against it. Concretely:

1. In `RETIRED.md`, replace "no longer the active development repository" with a
   statement that the **public Apache-2.0 release line** is frozen at `v0.1.24`
   and that the branch continues as the engine vendored by `physis-pro`, with
   previously granted rights unchanged.
2. In `docs/REPOSITORY_RETIREMENT.md`, correct the "last historical Apache
   commit" claim: the retirement documentation landed in `9426b91`, and commits
   continued after it on `origin/master`.
3. Decide explicitly whether future public Core is a new repository
   (`SPLIT_PLAN.md` says it is) and record the answer here.

If the intent is genuinely that the repository is retired, then the actionable
step is the opposite: stop committing to it and move the engine to its successor
repository. Committing to a repository the docs call retired is the state that
cannot continue.

## What I could not verify

- Whether `ilPez00/physis-core` is archived on GitHub, and the remote default
  branch beyond what `origin/master` shows locally (no network in this session).
- Which of local `master` (`3a7321a`) and `origin/master` (`ed8dc29`) is intended
  as the canonical branch; they have diverged.
