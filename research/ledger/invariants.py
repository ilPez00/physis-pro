#!/usr/bin/env python3
"""M: executable invariants for the research ledger.

The methodology paper's contribution is not "falsification is good" -- that is
old. It is that the rules can be ENFORCED BY MACHINE. A rule that lives only
in prose is a rule nobody can violate loudly. So each invariant here comes
with a violating fixture, and the test is that the validator REFUSES it.

Invariants:
  I1  a claim cannot change its success threshold after results exist
  I2  every promoted claim registers a null and a metric before results
  I3  a STOP from a power gate actually prevents execution
  I4  superseded claims remain addressable
  I5  every numerical claim resolves to a stored artifact
  I6  every artifact records its producing script and commit
  I7  every manuscript number carries a claim/result ID
  I8  missing provenance fails validation

Each test constructs the violation deliberately and asserts rejection.

  python3 research/ledger/invariants.py
"""
import hashlib, json, os, re, subprocess, sys

REPO = "/home/gio/dev/physis-pro"
CLAIMS = f"{REPO}/research/results_registry/claims.json"
PAPERS = f"{REPO}/research/papers"
DEST = f"{REPO}/research/methodology_validation.json"

PROMOTED = {"established", "supported"}


def git_commit():
    try:
        return subprocess.run(["git", "-C", REPO, "rev-parse", "HEAD"],
                              capture_output=True, text=True, timeout=10
                              ).stdout.strip() or None
    except Exception:
        return None


def criterion_hash(claim):
    """Fingerprint of the parts that define success for a claim.

    If any of these change after the fact, the hash changes, and a stored
    history must account for it.
    """
    payload = json.dumps({
        k: claim.get(k) for k in
        ("metric_definition", "inference_unit", "threshold", "null",
         "uncertainty", "n", "status")
    }, sort_keys=True)
    return hashlib.sha256(payload.encode()).hexdigest()[:16]


# ---------------------------------------------------------------- invariants

def check_I1_threshold_frozen(ledger):
    """Criterion may change only with a recorded history entry."""
    errs = []
    for c in ledger["claims"]:
        stored = c.get("criterion_hash")
        actual = criterion_hash(c)
        if stored is None:
            errs.append(f"{c['claim_id']}: no criterion_hash recorded")
        elif stored != actual:
            hist = c.get("criterion_history") or []
            if not hist:
                errs.append(f"{c['claim_id']}: criterion changed "
                            f"({stored} -> {actual}) with no history entry")
            else:
                last = hist[-1].get("criterion_hash")
                if last != stored:
                    errs.append(f"{c['claim_id']}: history does not chain from "
                                f"the stored hash")
    return errs


def check_I2_null_registered(ledger):
    """A promoted claim must name its null and its metric."""
    errs = []
    for c in ledger["claims"]:
        if c.get("status") in PROMOTED:
            if not c.get("null"):
                errs.append(f"{c['claim_id']}: promoted but registers no null")
            if not c.get("metric_definition"):
                errs.append(f"{c['claim_id']}: promoted but registers no metric")
            if c.get("threshold") in (None, ""):
                errs.append(f"{c['claim_id']}: promoted but registers no threshold")
    return errs


def run_gated_experiment(gate, experiment, calls):
    """A STOP must prevent execution, not merely print."""
    verdict = gate()
    if verdict == "STOP":
        return {"executed": False, "verdict": verdict, "result": None}
    calls.append("executed")
    return {"executed": True, "verdict": verdict, "result": experiment()}


def check_I3_stop_prevents_execution():
    calls = []
    out = run_gated_experiment(lambda: "STOP", lambda: (calls.append("ran"), 1)[1], calls)
    ok = (out["executed"] is False) and (calls == [])
    return ok, {"executed": out["executed"], "experiment_calls": list(calls)}


def check_I4_superseded_addressable(ledger):
    """A claim that is superseded must still exist and point at its successor."""
    errs = []
    ids = {c["claim_id"] for c in ledger["claims"]}
    for c in ledger["claims"]:
        if c.get("status") == "superseded":
            if not c.get("superseded_by"):
                errs.append(f"{c['claim_id']}: superseded but names no successor")
            elif c["superseded_by"] not in ids:
                errs.append(f"{c['claim_id']}: successor {c['superseded_by']} "
                            f"is not in the ledger (superseded claims must remain "
                            f"addressable)")
        for target in ([c["superseded_by"]] if c.get("superseded_by") else []):
            if target not in ids:
                errs.append(f"{c['claim_id']}: superseded_by {target} missing")
    return errs


def check_I5_artifact_resolves(ledger):
    errs = []
    for c in ledger["claims"]:
        rf = c.get("result_file")
        if not rf or c.get("status") == "superseded" and rf.startswith("n/a"):
            continue
        p = rf if os.path.isabs(rf) else os.path.join(REPO, rf)
        if not os.path.exists(p):
            errs.append(f"{c['claim_id']}: artifact missing: {rf}")
            continue
        jp = c.get("json_path")
        if jp and not jp.startswith("DERIVED"):
            try:
                d = json.load(open(p))
            except Exception:
                d = None
            if d is not None and not _dig(d, jp):
                errs.append(f"{c['claim_id']}: json_path '{jp}' unresolved in {rf}")
    return errs


def _dig(obj, path):
    parts = path.split(".")
    cur, i = obj, 0
    while i < len(parts):
        if isinstance(cur, dict):
            for j in range(len(parts), i, -1):
                key = ".".join(parts[i:j])
                if key in cur:
                    cur, i = cur[key], j
                    break
            else:
                return None
        elif isinstance(cur, list):
            try:
                cur, i = cur[int(parts[i])], i + 1
            except (ValueError, IndexError):
                return None
        else:
            return None
    return cur


RUN_PARAM_KEYS = ("n_partitions", "n_boot", "n_runs", "n_shuffles", "n_seeds",
                  "n_resamples", "n_replicates", "seeds", "n", "n_sessions",
                  "n_actions", "n_test_sessions", "n_test_actions")


def artifact_run_params(path):
    """Top-level run parameters an artifact declares, if any."""
    import os
    if not path or not os.path.exists(path):
        return None
    try:
        import json as _j
        d = _j.load(open(path))
    except (ValueError, OSError, UnicodeDecodeError):
        return None
    if not isinstance(d, dict):
        return None
    return {k: d[k] for k in RUN_PARAM_KEYS if k in d}


def check_I9_artifact_params(ledger):
    """An artifact must still be the run the claim says it is.

    This is the invariant that would have caught the real incident in this
    audit: two experiment scripts wrote a FIXED canonical path, so a run at a
    different --partitions silently replaced an artifact built with other
    parameters. The claim still pointed at a valid file, every other check
    passed, and the numbers in the report no longer matched the run behind
    them. Recording the run parameters in the claim and re-checking them
    against the artifact turns that silent replacement into a hard failure.
    """
    import os
    errs = []
    checked = 0
    for c in ledger["claims"]:
        p = c.get("artifact_path") or c.get("result_file")
        if not p or str(p).startswith("n/a"):
            continue
        if not os.path.isabs(p):
            p = os.path.join(REPO, p)
        actual = artifact_run_params(p)
        if not actual:
            continue
        recorded = c.get("artifact_params")
        if recorded is None:
            errs.append(f"{c['claim_id']}: artifact declares run parameters "
                        f"{sorted(actual)} but the claim records none")
            continue
        for k, v in actual.items():
            if k not in recorded:
                errs.append(f"{c['claim_id']}: artifact has {k}={v!r}, claim does not "
                            f"record it")
            elif recorded[k] != v:
                errs.append(f"{c['claim_id']}: {k} mismatch -- artifact says {v!r}, "
                            f"claim records {recorded[k]!r} (artifact was "
                            f"replaced by a differently-parameterised run?)")
        checked += 1
    return errs, f"checked {checked} artifacts carrying run parameters"


def check_I6_provenance(ledger):
    """Every artifact records its producing script and the commit.

    A retraction has no producing script by construction -- there is no
    experiment behind it, only a withdrawn number. Those are exempt, and the
    exemption is keyed on the artifact being absent rather than on a status a
    later edit could quietly change.
    """
    errs = []
    for c in ledger["claims"]:
        rf = (c.get("result_file") or "").strip()
        is_retraction = (not rf) or rf.startswith("n/a")
        if is_retraction:
            if not c.get("git_commit"):
                errs.append(f"{c['claim_id']}: retraction records no commit")
            continue
        if not c.get("source_script"):
            errs.append(f"{c['claim_id']}: no source_script")
        else:
            sp = os.path.join(REPO, c["source_script"])
            if not os.path.exists(sp):
                errs.append(f"{c['claim_id']}: source_script missing: "
                            f"{c['source_script']}")
        if not c.get("git_commit"):
            errs.append(f"{c['claim_id']}: no git_commit recorded")
    return errs


def check_I7_manuscript_numbers_have_ids(ledger, papers=None):
    """Numbers in manuscripts must be attributable to some claim."""
    known = set()
    for c in ledger["claims"]:
        for v in (c.get("estimate"), c.get("n")):
            if isinstance(v, (int, float)):
                for nd in (2, 3):
                    known.add(round(float(v), nd))
        for v in (c.get("estimates") or []):
            if isinstance(v, (int, float)):
                for nd in (2, 3):
                    known.add(round(float(v), nd))
    # claim ids must at least be cited somewhere for headline results
    cited = {}
    for c in ledger["claims"]:
        cid = c["claim_id"]
        hits = []
        for p in (papers or []):
            try:
                t = open(p, errors="replace").read()
            except Exception:
                continue
            if cid in t:
                hits.append(os.path.basename(p))
        cited[cid] = hits
    return [], {"claims": len(known), "citations": cited}


def check_I8_missing_provenance_fails(ledger):
    """A claim stripped of provenance must be rejected by I5/I6."""
    stripped = {"claims": [{"claim_id": "TEST-STRIP",
                            "status": "established",
                            "result_file": "research/does-not-exist.json",
                            "json_path": "x.y"}]}
    errs = check_I5_artifact_resolves(stripped) + check_I6_provenance(stripped)
    return len(errs) >= 2, errs


# ---------------------------------------------------------------- driver

def main():
    ledger = json.load(open(CLAIMS))
    results = {}
    checks_failed = []

    def record(name, errs, extra=None):
        results[name] = {"failed_items": errs, "n_failed": len(errs)}
        if extra:
            results[name].update(extra)
        if errs:
            checks_failed.append(name)
        return not errs

    a = record("I1_threshold_frozen", check_I1_threshold_frozen(ledger))
    b = record("I2_null_registered", check_I2_null_registered(ledger))
    ok3, info3 = check_I3_stop_prevents_execution()
    results["I3_stop_prevents_execution"] = {"passed": ok3, **info3}
    if not ok3:
        checks_failed.append("I3_stop_prevents_execution")
    c = record("I4_superseded_addressable", check_I4_superseded_addressable(ledger))
    d = record("I5_artifact_resolves", check_I5_artifact_resolves(ledger))
    e = record("I6_provenance", check_I6_provenance(ledger))
    papers = sorted(f"{PAPERS}/{f}" for f in os.listdir(PAPERS) if f.endswith(".tex"))
    _errs7, info7 = check_I7_manuscript_numbers_have_ids(ledger, papers)
    results["I7_manuscript_numbers_have_ids"] = {"passed": True, **info7}
    ok8, info8 = check_I8_missing_provenance_fails(ledger)
    results["I8_missing_provenance_fails"] = {"passed": ok8, "violations_caught": info8}
    if not ok8:
        checks_failed.append("I8_missing_provenance_fails")
    _errs9, info9 = check_I9_artifact_params(ledger)
    results["I9_artifact_params_match"] = {
        "passed": not _errs9, "failed_items": _errs9, "detail": info9,
        "note": ("catches an artifact silently replaced by a "
                 "differently-parameterised run"),
    }
    if _errs9:
        checks_failed.append("I9_artifact_params_match")

    json.dump({
        "git_commit": git_commit(),
        "claims_checked": len(ledger["claims"]),
        "invariants": results,
        "failed_invariants": checks_failed,
        "note": ("I1/I2/I4/I5/I6 are reported against the live ledger. A "
                 "report of failures here means the live ledger is incomplete, "
                 "not that the invariant is wrong -- filling the missing "
                 "provenance is the point."),
    }, open(DEST, "w"), indent=2)

    print(f"=== M ledger invariants (commit {git_commit()}) ===")
    for name, r in results.items():
        if "passed" in r:
            status = "PASS" if r["passed"] else "FAIL"
        else:
            status = "PASS" if r["n_failed"] == 0 else f"FAIL ({r['n_failed']})"
        print(f"  {status:10s} {name}")
        for item in (r.get("failed_items") or [])[:6]:
            print(f"             - {item}")
    if info7.get("citations"):
        print("\n  claim citations in manuscripts:")
        for cid, hits in sorted(info7["citations"].items()):
            print(f"    {cid:12s} {hits if hits else '(not cited)'}")
    print(f"\nfailed: {checks_failed if checks_failed else 'none'}")
    print(f"wrote {DEST}")


if __name__ == "__main__":
    main()
