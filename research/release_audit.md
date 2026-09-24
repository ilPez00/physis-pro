# Release security audit

Artifact: `benchmarks/behaviour/corpus-redacted.json.gz` (the released corpus)
plus the raw captures it is derived from.
Scanner: `research/release_audit.py`  ·  Scrubber: `research/scrub_corpus.py`
Date: 2026-09-24

**Verdict: the release was blocked, and remains blocked pending the three
items in §6.** Credential-class material was found inside the artifact that
was described as redacted. It has been removed from the working tree; the
tokens themselves must still be treated as compromised.

---

## 1. What was found

### 1.1 Critical — a live-shaped NVIDIA API key inside the redacted release

`corpus-redacted.json.gz` hashes `action.target`, but `action.kind` is
*derived from the command text*, so a secret that survived into a kind label
survived redaction. One key did:

| | |
|---|---|
| occurrences | 2 (session 13, actions 568 and 569) |
| field | `action.kind` |
| shape | `nvapi-` + 65 chars |
| matches the currently-active nvidia key? | **no** (different SHA-256) |

It is an older key. It does not match the key in `~/.hermes/config.yaml`, so
it is not the credential in current use — but it is credential-shaped, it was
never truncated, and the artifact is intended for public release.

### 1.2 Critical — a GitHub PAT and full command text in the raw captures

The raw captures (`actions.json`, `actions-merged.json`, `actions-pansa.json`,
`actions-omo.json`, `actions-feather.json`) store **full shell command text**
in `action.target`. Found there, unredacted:

- a GitHub personal access token (`ghp_` + 36 chars) — 25 occurrences across
  3 files, appearing inside commands such as
  `cd /home/gio/gh-profile && GH_TOKEN=<token> GITHUB_TOKEN=<token>`
- a command that reads a token out of `~/.env` and pipes it through `sed`
- absolute paths containing the account name, e.g. `/home/gio/.env`

These files are **tracked in git and pushed**. They are not part of the
released corpus, but they are part of the repository.

### 1.3 Informational — structural fields that identify the operator

| field | distinct | example class |
|---|---|---|
| `project` | 10 | `-home-gio-Praxis-praxis-webapp`, `-home-gio-dev-physis-pro`, `-home-gio-Scaricati` |
| `machine` | 3 | `omo`, `feather`, `pansa` |
| `corpus` | 11 | `opencode:omo`, `cline:feather` |
| `seen_on` | 11 | as above |

`project` leaks the account name, four real project names, and a local
directory name. `machine`/`corpus`/`seen_on` leak hostnames. None of these is
a credential; all of them are identifying, and the papers describe the release
as redacted.

### 1.4 Redaction claim checked

`action.target`: 75,933 values, **70,100 hashed (92.3%)**, **5,833 empty**
(the command was unavailable), 0 neither. The hashing claim holds for every
non-empty value. The 5,833 empties are absence, not leakage, and the paper
should say so rather than implying 100% hashing.

---

## 2. Exposure assessment

| question | answer |
|---|---|
| Is the release public? | no — the corpus has not been published |
| Is the repository public? | **no** — `ilPez00/physis-pro` is **private** |
| Are the secrets in pushed history? | **yes** — present on remote-tracking refs `aa0cbf62`, `f85d2781`, `3538746a` |
| Last push | 2026-09-23 |
| Public exposure? | none found |

The tokens have not been published. They do exist on GitHub in a private
repository, so they are exposed to anyone with repository access, to GitHub
itself, and to any future change in repository visibility.

---

## 3. What was done

`research/scrub_corpus.py` replaced every credential-shaped token in the
working tree — 39 replacements across 6 files:

```
actions-feather.json      5
actions-merged.json       9
actions-omo.json          5
actions-pansa.json        9
actions.json              9
corpus-redacted.json.gz   2
```

Replacements use a fixed `<SECRET-REDACTED>` placeholder, so labels that
differed only by an embedded secret collapse rather than fragmenting the
vocabulary. Verified afterwards: **0 secret-shaped tokens remain**, and the
derived numbers are unchanged — 75,933 actions, 402 sessions, 871 distinct
kinds, registry validator still 16/16.

The scrubbed `.gz` is written with `mtime=0` so the artifact is
byte-reproducible. The scrubber is idempotent and self-checking.

---

## 4. What is still true and must not be glossed

- **The tokens must be rotated.** Scrubbing the working tree does not
  invalidate a credential that already existed. The GitHub PAT in particular
  granted repository access and should be revoked at GitHub regardless of
  whether it still works. The old NVIDIA key should be revoked at NVIDIA.
  This audit deliberately did **not** validate either token by using it.
- **Git history still contains them.** Any future public release requires
  history rewriting (e.g. `git filter-repo`) or publishing from a fresh
  repository. A commit that deletes the string does not remove it from
  history.
- **The raw captures violate the programme's own rule.** The brief states
  "do not publish raw session transcripts". `actions*.json` *are* raw
  transcripts with command text intact. They should be untracked and
  gitignored, with only the derived, hashed corpus published.

---

## 5. Recommended change to the corpus pipeline

Redaction currently hashes `target` but derives `kind` from raw command text,
which is how §1.1 happened. The fix belongs in generation, not in an
after-the-fact scrub:

1. run the secret scrub **before** deriving `kind` labels, so no label can
   ever be built from a credential;
2. hash or drop `project`, `machine`, `corpus` and `seen_on` (or coarsen them
   to a stable pseudonym), and state the treatment in the paper;
3. add `research/release_audit.py` as a **gate** that fails the build if any
   secret-shaped token appears in a release artifact;
4. add a unit check that no released field contains `/home/` or a bare
   hostname from the fleet.

Until 1 and 2 are implemented, `corpus-redacted.json.gz` should carry a
release note recording that it is scrubbed post hoc rather than scrubbed
by construction.

---

## 6. Release blockers

1. **Rotate** the GitHub PAT and the old NVIDIA key. (Operator action.)
2. **Scrub at generation, not after.** The raw captures are consumed by
   `extract_actions.py`, `merge.py`, `e71.py`, `e72.py`, `e73.py`,
   `validate_predict.py` and `research/criticality/run.py`, so merely
   untracking them would break the analysis pipeline without removing them
   from history. The durable fix is §5: scrub before `kind` derivation, hash
   the identifying structural fields, and let a gate reject any release
   artifact that still matches a secret pattern.
3. **Hash or pseudonymise** `project`, `machine`, `corpus`, `seen_on`, and say
   so in the dataset paper.

Item 1 is the operator's; 2 and 3 are code changes in the corpus build. Until
they land, the repository must stay private and `corpus-redacted.json.gz` must
not be published.
