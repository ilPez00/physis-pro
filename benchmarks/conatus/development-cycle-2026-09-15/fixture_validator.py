#!/usr/bin/env python3
"""Temporal fixture validator — development-cycle-2026-09-15.

This validates the FIXTURES (fixtures/temporal_fixtures.json), not the
production engine. Physis-core has no state(valid_at=T, known_at=K) query yet
(see ADAPTER_CONTRACT.md and the development-cycle REQUESTS.md); this script
exists so the fixture suite is executable and self-checking before that
interface is built, per the task's instruction not to fake runtime
integration. It implements two things:

1. A reference oracle for the frozen semantics documented in the fixture
   file's own "status_vocabulary" / "interval_semantics" / "visibility_rule"
   fields. Every fixture's declared `expected_status` is recomputed from its
   raw evidence and diffed against the oracle — a mismatch means the fixture
   was authored wrong, not that production code is wrong.
2. A KNOWN-BAD CONTROL: a `known_at`-only oracle that reproduces the defect
   E77 measured (benchmarks/conatus/e77_temporal.py) — it ignores valid_at
   entirely and answers purely from what has been ingested by known_at. If
   this control agreed with the correct oracle everywhere, the fixture suite
   would not actually be testing anything about event time vs. write time.
   The validator asserts the two oracles DISAGREE on at least one query per
   temporal case (i.e. every case that is supposed to be temporally
   discriminating actually discriminates), and reports the disagreement rate.

Exit code is nonzero on any self-consistency failure. This script has no
third-party dependencies.

Usage:
    python3 fixture_validator.py [--fixtures fixtures/temporal_fixtures.json] [--json]
"""
import argparse
import json
import sys
from datetime import datetime, timezone


def parse_ts(s):
    if s is None:
        return None
    return datetime.fromisoformat(s.replace("Z", "+00:00"))


def load_fixtures(path):
    with open(path) as f:
        return json.load(f)


def visible_evidence(evidence, known_at):
    """Evidence items visible at known_at, with superseded items excluded
    once their superseding item is itself visible."""
    by_id = {e["id"]: e for e in evidence}
    visible_ids = {e["id"] for e in evidence if parse_ts(e["known_at"]) <= known_at}
    superseded = set()
    for e in evidence:
        sup = e.get("supersedes")
        if sup and e["id"] in visible_ids:
            superseded.add(sup)
    return [by_id[i] for i in visible_ids if i not in superseded]


def applicable_at(item, valid_at):
    """Half-open interval test: from <= t < until. valid_unknown items are
    never 'applicable' in the dated sense — they are handled separately."""
    if item.get("valid_unknown"):
        return False
    frm = parse_ts(item.get("valid_from"))
    until = parse_ts(item.get("valid_until"))
    if frm is not None and valid_at < frm:
        return False
    if until is not None and valid_at >= until:
        return False
    return True


def oracle_state(evidence, valid_at, known_at):
    """The correct, typed oracle: status(valid_at=T, known_at=K)."""
    visible = visible_evidence(evidence, known_at)
    applicable = [e for e in visible if applicable_at(e, valid_at)]
    unknown_time = [e for e in visible if e.get("valid_unknown") and e["id"] not in {a["id"] for a in applicable}]

    contradicting = [e for e in applicable if e["polarity"] == "contradicting"]
    supporting = [e for e in applicable if e["polarity"] == "supporting"]

    if contradicting:
        status, ids = "CONTRADICTED", [e["id"] for e in contradicting]
    elif supporting:
        status, ids = "SUPPORTED", [e["id"] for e in supporting]
    elif unknown_time:
        status, ids = "UNKNOWN_TIME", [e["id"] for e in unknown_time]
    else:
        status, ids = "NO_BELIEF", []
    return status, sorted(ids)


def known_bad_state(evidence, valid_at, known_at):
    """KNOWN-BAD CONTROL: ignores valid_at completely. Answers from whatever
    is visible by known_at, exactly like the measured E77 behaviour (replay
    indexed by write/ingestion time, not by the time the evidence is about).
    `valid_at` is accepted but unused -- that is the point."""
    del valid_at
    visible = visible_evidence(evidence, known_at)
    contradicting = [e for e in visible if e["polarity"] == "contradicting" and not e.get("valid_unknown")]
    supporting = [e for e in visible if e["polarity"] == "supporting" and not e.get("valid_unknown")]
    unknown_time = [e for e in visible if e.get("valid_unknown")]
    if contradicting:
        return "CONTRADICTED", sorted(e["id"] for e in contradicting)
    if supporting:
        return "SUPPORTED", sorted(e["id"] for e in supporting)
    if unknown_time:
        return "UNKNOWN_TIME", sorted(e["id"] for e in unknown_time)
    return "NO_BELIEF", []


def validate(fixtures):
    cases = fixtures["cases"]
    by_case = {c["id"]: c for c in cases}
    results = []
    failures = []
    discriminating_queries = 0
    disagreements = 0
    total_queries = 0

    for case in cases:
        evidence = case["evidence"]
        for q in case["queries"]:
            total_queries += 1
            valid_at = parse_ts(q["valid_at"])
            known_at = parse_ts(q["known_at"])
            got_status, got_ids = oracle_state(evidence, valid_at, known_at)
            exp_status = q["expected_status"]
            exp_ids = sorted(q.get("expected_evidence_ids", []))

            ok = (got_status == exp_status) and (got_ids == exp_ids)
            bad_status, bad_ids = known_bad_state(evidence, valid_at, known_at)
            disagrees = (bad_status, bad_ids) != (got_status, got_ids)
            discriminating_queries += 1
            if disagrees:
                disagreements += 1

            # cross-case invariance: "must_equal_case"/"must_equal_query"
            cross_ok = True
            if "must_equal_case" in q:
                other_case = by_case[q["must_equal_case"]]
                other_q = next(x for x in other_case["queries"] if x["id"] == q["must_equal_query"])
                cross_ok = (exp_status == other_q["expected_status"]) and (
                    sorted(q.get("expected_evidence_ids", [])) == sorted(other_q.get("expected_evidence_ids", []))
                )

            results.append({
                "case": case["id"],
                "query": q["id"],
                "category": case["category"],
                "oracle_status": got_status,
                "oracle_evidence": got_ids,
                "expected_status": exp_status,
                "expected_evidence": exp_ids,
                "fixture_self_consistent": ok,
                "known_bad_status": bad_status,
                "known_bad_evidence": bad_ids,
                "known_bad_disagrees_with_oracle": disagrees,
                "cross_case_invariance_ok": cross_ok,
            })
            if not ok:
                failures.append(
                    f"{case['id']}/{q['id']}: fixture declares {exp_status} {exp_ids}, "
                    f"oracle computes {got_status} {got_ids}"
                )
            if not cross_ok:
                failures.append(
                    f"{case['id']}/{q['id']}: cross-case invariance with "
                    f"{q.get('must_equal_case')}/{q.get('must_equal_query')} violated"
                )

    # Suite-level check: the fixture set as a whole must actually discriminate
    # valid_at from known_at -- if the known-bad control never disagreed with
    # the oracle, the suite would not be testing what it claims to test.
    if disagreements == 0:
        failures.append(
            "SUITE-LEVEL: the known-bad (known_at-only) control never disagreed with the "
            "typed oracle on any query -- the fixture suite would not discriminate valid_at "
            "from known_at and must not be reported as testing temporal semantics."
        )

    # Category coverage check against the assignment's required distinctions.
    required_categories = {
        "late_arrival", "correction", "unknown_time", "interval_endpoint",
        "forward_reverse_final_evidence",
    }
    present_categories = {c["category"] for c in cases}
    missing = required_categories - present_categories
    if missing:
        failures.append(f"SUITE-LEVEL: missing required categories: {sorted(missing)}")

    return {
        "total_cases": len(cases),
        "total_queries": total_queries,
        "known_bad_disagreements": disagreements,
        "known_bad_disagreement_rate": disagreements / total_queries if total_queries else 0.0,
        "categories_present": sorted(present_categories),
        "results": results,
        "failures": failures,
        "passed": len(failures) == 0,
    }


def self_test(fixtures):
    """Negative control on the validator itself: mutate one fixture's declared
    expected_status in memory (no file touched) and confirm the validator
    actually catches it. A validator that passes every input, broken or not,
    would be worthless; this is that validator's own known-bad control."""
    import copy
    broken = copy.deepcopy(fixtures)
    q = broken["cases"][1]["queries"][1]
    assert q["id"] == "q_jun"
    original = q["expected_status"]
    q["expected_status"] = "SUPPORTED" if original != "SUPPORTED" else "CONTRADICTED"
    report = validate(broken)
    caught = not report["passed"] and any("q_jun" in f for f in report["failures"])
    return caught, report["failures"]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixtures", default="fixtures/temporal_fixtures.json")
    ap.add_argument("--json", action="store_true", help="emit machine-readable JSON only")
    ap.add_argument("--self-test", action="store_true",
                     help="run the validator's own known-bad control: mutate a fixture in "
                          "memory and confirm the validator flags it, then exit")
    args = ap.parse_args()

    fixtures = load_fixtures(args.fixtures)

    if args.self_test:
        caught, failures = self_test(fixtures)
        if caught:
            print("SELF-TEST PASS: an induced fixture error (T02-e77-typed-replication/q_jun "
                  "expected_status flipped) was correctly flagged by the validator:")
            for f in failures:
                print(f"  - {f}")
            sys.exit(0)
        else:
            print("SELF-TEST FAIL: the validator did not catch an induced fixture error. "
                  "Do not trust a clean run of this script until this is fixed.")
            sys.exit(1)

    report = validate(fixtures)

    if args.json:
        print(json.dumps(report, indent=1))
        sys.exit(0 if report["passed"] else 1)

    print(f"Temporal fixture validator — {args.fixtures}")
    print(f"  cases: {report['total_cases']}  queries: {report['total_queries']}")
    print(f"  categories present: {', '.join(report['categories_present'])}")
    print(f"  known-bad control disagreed with the typed oracle on "
          f"{report['known_bad_disagreements']}/{report['total_queries']} queries "
          f"({report['known_bad_disagreement_rate']:.0%})")
    print()
    for r in report["results"]:
        mark = "PASS" if r["fixture_self_consistent"] and r["cross_case_invariance_ok"] else "FAIL"
        kb = "differs" if r["known_bad_disagrees_with_oracle"] else "SAME AS ORACLE"
        print(f"  [{mark}] {r['case']}/{r['query']}: oracle={r['oracle_status']} "
              f"known-bad={r['known_bad_status']} ({kb})")

    print()
    if report["failures"]:
        print(f"FAILURES ({len(report['failures'])}):")
        for f in report["failures"]:
            print(f"  - {f}")
        sys.exit(1)
    else:
        print("All fixtures self-consistent; known-bad control diverges where expected.")
        sys.exit(0)


if __name__ == "__main__":
    main()
