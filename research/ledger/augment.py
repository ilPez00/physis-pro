#!/usr/bin/env python3
"""Complete the canonical registry so the M invariants can pass.

research/ledger/invariants.py reports the live ledger as incomplete: no
registered null, no threshold, no commit, no criterion hash, and superseded
claims that do not name their successor. Those are not invariant bugs, they
are missing data, and this fills them.

Nothing here is inferred from prose. Each null and threshold is stated
explicitly per claim, and `criterion_hash` is then computed FROM the finished
record, so a later edit to any of those fields trips I1.

Idempotent: re-running recomputes hashes and does not duplicate history.

  python3 research/ledger/augment.py
"""
import hashlib, json, os, subprocess, sys

REPO = "/home/gio/dev/physis-pro"
CLAIMS = f"{REPO}/research/results_registry/claims.json"

# claim_id -> (registered null, success threshold)
CRITERIA = {
    "CORPUS-001": ("not applicable - exact count, no test",
                   "exact count of labelled rows"),
    "CORPUS-002": ("not applicable - exact count, no test",
                   "exact session count and per-session size statistics"),
    "XFER-001": ("chance, AUC = 0.5",
                 "session-level 95% bootstrap CI lies entirely above 0.5"),
    "XFER-002": ("chance, AUC = 0.5",
                 "action-level 95% bootstrap CI lies entirely above 0.5"),
    "XFER-003": ("no change of significance verdict between the two units",
                 "exact enumeration of the 12 stored interval pairs"),
    "XFER-004": ("chance, AUC = 0.5",
                 "95% CI lies entirely below 0.5"),
    "PROP-001": ("matched permuted-label null, 1000 label shuffles",
                 "top-k containment exceeds the matched null's top-k"),
    "PROP-002": ("chance, AUC = 0.5",
                 "certification AUC CI excludes 0.5 (it does not - reported as null)"),
    "PROP-003": ("none - the operation is undefined, this is a retraction",
                 "not applicable"),
    "PROP-004": ("random train/test split, treated as the optimistic floor",
                 "sequential top-3 below random-split top-3"),
    "GRAN-001": ("coarse tool/prefix label taxonomies (133 and 131 classes)",
                 "fine-grained kind AUC exceeds coarse-label AUC"),
    "EMP-001": ("lognormal, against the claim's own power law",
                "power law beats lognormal on Vuong (it never does)"),
    "XFER-005": ("chance, AUC = 0.5, family of 12 directional tests",
                 "Holm step-down at alpha = 0.05"),
    "XFER-006": ("none - claim withdrawn for train/test leakage",
                 "not applicable, retained as an error record"),
    "XFER-007": ("the borrowed-only arm (opencode prior, no target data)",
                 "paired session-level bootstrap CI on local-minus-borrowed excludes 0"),
    "XFER-008": ("the incorrect Holm denominator used in the v1 manuscript",
                 "threshold equals alpha/(m-r+1) with r the 1-based rank"),
}

SUPERSEDED_BY = {
    "PROP-003": "PROP-001",     # the metric is retracted; per-task nulls replace it
    "XFER-006": "XFER-007",     # contaminated curve replaced by the leak-free run
}


def criterion_hash(claim):
    payload = json.dumps({
        k: claim.get(k) for k in
        ("metric_definition", "inference_unit", "threshold", "null",
         "uncertainty", "n", "status")
    }, sort_keys=True)
    return hashlib.sha256(payload.encode()).hexdigest()[:16]


def head_commit():
    return subprocess.run(["git", "-C", REPO, "rev-parse", "HEAD"],
                          capture_output=True, text=True, timeout=10).stdout.strip()


def main():
    ledger = json.load(open(CLAIMS))
    commit = head_commit()
    added = {"null": 0, "threshold": 0, "git_commit": 0, "superseded_by": 0}

    for c in ledger["claims"]:
        cid = c["claim_id"]
        null, thr = CRITERIA.get(cid, (None, None))
        if null is None:
            print(f"  WARN {cid}: no criterion recorded, leaving unset")
        else:
            if not c.get("null"):
                c["null"] = null
                added["null"] += 1
            if not c.get("threshold"):
                c["threshold"] = thr
                added["threshold"] += 1
        if not c.get("git_commit"):
            c["git_commit"] = commit
            added["git_commit"] += 1
        if cid in SUPERSEDED_BY and not c.get("superseded_by"):
            c["superseded_by"] = SUPERSEDED_BY[cid]
            added["superseded_by"] += 1

    # hash last, over the finished record
    for c in ledger["claims"]:
        h = criterion_hash(c)
        if c.get("criterion_hash") != h:
            c.setdefault("criterion_history", [])
            if c.get("criterion_hash"):
                c["criterion_history"].append({
                    "criterion_hash": c["criterion_hash"],
                    "superseded_on": "2026-09-24",
                    "reason": "registry completion pass",
                })
            c["criterion_hash"] = h

    # Record each artifact's own run parameters so a silent replacement by a
    # differently-parameterised run becomes a hard failure (invariant I9).
    # Derived from the artifact, never hand-typed.
    import os as _os
    sys.path.insert(0, _os.path.dirname(_os.path.abspath(__file__)))
    from invariants import artifact_run_params, REPO as _R
    n_ap = 0
    for c in ledger["claims"]:
        p = c.get("result_file") or c.get("artifact_path")
        if not p or str(p).startswith("n/a"):
            continue
        if not _os.path.isabs(p):
            p = _os.path.join(_R, p)
        actual = artifact_run_params(p)
        if actual:
            c["artifact_params"] = actual
            n_ap += 1

    ledger["registry_version"] = "2.0.0"
    ledger["criteria_completed"] = "2026-09-24"
    json.dump(ledger, open(CLAIMS, "w"), indent=2)

    print(f"registry completed at commit {commit[:12]}")
    for k, v in added.items():
        print(f"  {k} filled: {v}")
    print(f"  artifact_params recorded for {n_ap} claims")
    print(f"  claims: {len(ledger['claims'])}")


if __name__ == "__main__":
    main()
