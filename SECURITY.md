# Security Policy

## Reporting a vulnerability

Report privately through GitHub's private vulnerability reporting on this
repository — **Security → Report a vulnerability**. That channel keeps the
report unreadable to anyone but the maintainers until a fix ships, which a
public issue does not.

Please do not open a public issue for a suspected vulnerability.

A useful report says which version you tested, what an attacker controls, and
what they achieve. A proof of concept helps but is not required; a precise
description of the reachable path is worth more than a script.

## Repository history rewrite (2026-10-26)

Before this repository was made public, its history was rewritten to remove a
licence signing key (`.physis-vendor/`) and leaked credentials from research
corpora. Treat the removed key as compromised: rotate it before issuing any new
licence. Commit hashes from before the rewrite are not valid against this
repository; delete old clones and re-clone with `git clone --recursive`.

## Supported versions

Only the latest published release of `physis_pro` receives security fixes.
Older releases are not backported — the fix ships in a new version and the
affected ones are yanked.

| Crate          | Supported            |
| -------------- | -------------------- |
| `physis_pro`   | latest release only  |
| `physis-core`  | latest release only  |

Yanking blocks *new* dependents from resolving to an affected version. It does
not remove it: a project with an existing `Cargo.lock` pinning that version
keeps building it, and keeps the vulnerability. Upgrading is the remediation;
the yank only stops the spread.

## Known advisories

| Advisory | Crate | Affected | Fixed in | Summary |
| -------- | ----- | -------- | -------- | ------- |
| [provider availability](security/advisories/physis_pro-provider-availability.md) | `physis_pro` | `>=0.1.6, <0.1.9` | `0.1.9` | A remote error body could permanently disable a shared LLM provider for every tenant |

Drafts in `security/advisories/` are the text intended for submission to the
[RustSec advisory database](https://github.com/rustsec/advisory-db), so that
`cargo audit` reaches deployments that a yank alone cannot.
