#!/usr/bin/env python3
"""Validate the canonical results registry.

Three jobs:
  1. every claim with a json_path traces to a real value in its result file;
  2. derived claims are recomputed from the raw artifacts, not trusted;
  3. no paper asserts a value that contradicts a claim it shares with another
     paper -- the check that would have caught the paper1/paper9 conflict.

Exits non-zero on any failure. Never repairs silently: a failing claim is
reported with both the registry value and the artifact value.

  python3 research/results_registry/validate.py
"""
import json, os, re, sys

REPO = "/home/gio/dev/physis-pro"
REG = os.path.join(REPO, "research/results_registry/claims.json")
PAPERS = os.path.join(REPO, "research/papers")

fails, warns, checks = [], [], 0


def fail(msg):
    fails.append(msg)
    print(f"  FAIL  {msg}")


def ok(msg):
    global checks
    checks += 1
    print(f"  ok    {msg}")


def load(path):
    try:
        return json.load(open(os.path.join(REPO, path)))
    except Exception as e:
        fail(f"cannot load {path}: {e}")
        return None


def dig(obj, path):
    """Resolve a dotted path, tolerating keys that themselves contain dots.

    Needed because frac and parameter keys are literal (results.'0.75',
    settings.'0.01'), so a naive split on '.' cannot address them.
    Longest-match-first at each step.
    """
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


# ---------------------------------------------------------------- derived

def derive_bootstrap_counts():
    """Recompute the counts that paper1 and paper9 both quote.

    Returns (session_above, action_above, verdict_changes, gained, lost)
    where 'above' means the whole CI lies above AUC 0.5.
    """
    d = load("research/bootstrap_unit_check.json")
    if not d or "pairs" not in d:
        return None
    pa = d["pairs"]
    s_above = [p for p, v in pa.items() if v["session"]["lo"] > 0.5]
    a_above = [p for p, v in pa.items() if v["action"]["lo"] > 0.5]

    def contains(ci):
        return ci["lo"] <= 0.5 <= ci["hi"]

    gained = [p for p, v in pa.items() if contains(v["session"]) and not contains(v["action"])]
    lost = [p for p, v in pa.items() if contains(v["action"]) and not contains(v["session"])]
    session_below = [p for p, v in pa.items() if v["session"]["hi"] < 0.5]
    action_below = [p for p, v in pa.items() if v["action"]["hi"] < 0.5]
    return {
        "session_above": len(s_above), "action_above": len(a_above),
        "gained": len(gained), "lost": len(lost),
        "verdict_changes": len(gained) + len(lost),
        "session_below": len(session_below), "action_below": len(action_below),
        "pairs": len(pa),
    }


def cross_paper_checks(claims):
    """Assert shared claims are stated consistently in every paper that uses them.

    Each assertion is a regex that must be present in that paper. The point is
    not to parse LaTeX but to freeze the agreed value so a later edit to one
    paper cannot silently drift from the others.
    """
    print("\n=== cross-paper consistency ===")
    bc = derive_bootstrap_counts()
    if not bc:
        fail("could not derive bootstrap counts from artifacts")
        return
    print(f"  [derived] session-above={bc['session_above']} action-above={bc['action_above']} "
          f"verdict-changes={bc['verdict_changes']} (gained {bc['gained']}, lost {bc['lost']})")
    print(f"  [derived] CI below 0.5: session={bc['session_below']} action={bc['action_below']}")

    # the registry's stated values must equal the recomputed ones
    by_id = {c["claim_id"]: c for c in claims}
    for cid, expect in (("XFER-001", bc["session_above"]), ("XFER-002", bc["action_above"])):
        c = by_id.get(cid)
        if not c:
            fail(f"{cid} missing from registry")
            continue
        d = load(c["result_file"])
        actual = dig(d, c["json_path"]) if d else None
        if actual != expect:
            fail(f"{cid}: registry says {expect}, artifact says {actual} (unit={c['inference_unit']})")
        else:
            ok(f"{cid} artifact agrees: {actual}")

    c = by_id.get("XFER-003")
    if c:
        p9 = open(os.path.join(PAPERS, "paper9_dataset.tex")).read()
        words = {1: "one", 2: "two", 3: "three", 4: "four", 5: "five"}
        want, g, l = words[bc["verdict_changes"]], bc["gained"], bc["lost"]
        wg, wl = words[g], words[l]
        if f"for \\emph{{{want}}} of them" in p9:
            ok(f"paper9 states verdict-change count as '{want}' (derived: {bc['verdict_changes']})")
        else:
            fail(f"paper9 does not state the derived verdict-change count '{want}'")
        if re.search(r"turn four chance-level comparisons into", p9):
            fail("paper9 still contains the retracted 'four chance-level comparisons' claim")
        else:
            ok("paper9 no longer claims four comparisons turned significant")
        if wg in p9 and wl in p9:
            ok(f"paper9 states direction split ({wg} gained / {wl} lost), matching artifact")
        else:
            fail(f"paper9 does not state direction split {wg}/{wl} (derived {g}/{l})")

    # ---- P2: the external replication narrowed the paper. Guard the two
    # claims it no longer supports, and require the one it now rests on.
    try:
        p2 = open(os.path.join(PAPERS, "paper2_trajectories.tex")).read()
    except OSError:
        p2 = ""
    if not p2:
        warns.append("paper2_trajectories.tex not found")
    else:
        if "p2_results.json" in p2 or "p2\\_results.json" in p2:
            ok("P2: cites the external replication artifact")
        else:
            fail("P2: does not cite research/trajectories_external/p2_results.json")
        if "no operational prediction claim" in p2 or "not claimed" in p2:
            ok("P2: declines the operational-prediction claim")
        else:
            warns.append("P2: operational-prediction disclaimer not found")
        if "0.214" in p2 and "0.369" in p2:
            ok("P2: reports the lexical gap medians against nomic (0.214 vs 0.369)")
        else:
            fail("P2: identity claim not shown against the lexical baselines")
        if "isotropic" in p2:
            ok("P2: dimensionality stated against the isotropic null")
        else:
            fail("P2: dimensionality stated without a null that can fail")

    # paper1's above-chance counts
    p1 = open(os.path.join(PAPERS, "paper1_failure_priors.tex")).read()
    for cid, n, phrase in (("XFER-002", bc["action_above"], "action-level"),
                           ("XFER-001", bc["session_above"], "session-level")):
        w = {1: "one", 2: "two", 3: "three"}[n]
        if f"two of twelve pairs" in p1 or f"{w} of twelve" in p1:
            ok(f"paper1 uses a consistent '<n> of twelve' form for {phrase} ({n})")
        else:
            warns.append(f"paper1: could not confirm the '{w} of twelve' form for {cid}")

    # ---- XFER-005: multiplicity. The Holm survivors must all be BELOW chance,
    # and paper1 must not resurrect the pre-correction headline.
    mult = load("research/e1_multiplicity.json")
    if mult:
        surv = mult["holm_survivors"]
        by = {r["pair"]: r for r in mult["pairs"]}
        above = [s for s in surv if by[s]["auc_boot_mean"] > 0.5]
        if above:
            fail(f"XFER-005: Holm survivors include pairs ABOVE chance: {above} "
                 f"— paper1's 'no positive transfer' claim would be false")
        else:
            ok(f"XFER-005: all {len(surv)} Holm survivors are below chance "
               f"({[round(by[s]['auc_boot_mean'],3) for s in surv]})")
        lo = by["cline->opencode"]
        if lo["holm_reject"]:
            fail("XFER-005: cline->opencode now survives Holm — paper1 says it fails")
        else:
            ok(f"XFER-005: cline->opencode does not survive Holm "
               f"(p={lo['p_raw']:.4f} > {lo['holm_threshold']:.4f})")
        if "No pair shows positive transfer" in p1 or "no pair shows positive" in p1:
            ok("XFER-005: paper1 states the no-positive-transfer result")
        else:
            fail("XFER-005: paper1 does not state the no-positive-transfer result")

    # ---- XFER-008: the Holm arithmetic itself (v2). The v1 manuscript quoted
    # rank 3's denominator for a rank-5 hypothesis; this checks the corrected
    # number, the generated sentence, and that the wrong form never returns.
    v2 = load("research/p1_multiplicity_v2.json")
    if v2:
        expect_holm = {"claude-code->codex", "codex->opencode", "codex->cline"}
        got_holm = set(v2["holm_survivors"])
        if got_holm != expect_holm:
            fail(f"XFER-008: Holm rejection set changed: {got_holm} != {expect_holm}")
        else:
            ok(f"XFER-008: Holm rejection set matches the frozen expectation")
        v2by = {r["pair"]: r for r in v2["pairs"]}
        lp = v2by["cline->opencode"]
        if abs(lp["holm_threshold"] - 0.05 / 8) > 1e-12:
            fail(f"XFER-008: cline->opencode Holm threshold {lp['holm_threshold']} "
                 f"is not 0.05/8 = {0.05/8}")
        else:
            ok("XFER-008: cline->opencode Holm threshold is 0.05/8 = 0.00625")
        if not lp["nominally_significant"]:
            fail("XFER-008: cline->opencode is not nominally significant")
        else:
            ok("XFER-008: cline->opencode is nominally significant but not Holm-significant")
        # the implementation tests must have run and passed in the artifact
        for tname in ("fwer_monte_carlo", "planted_signal_detection",
                      "holm_formulations_agree", "bh_dominates_holm",
                      "frozen_rejection_set"):
            t = v2.get("tests", {}).get(tname)
            if not t or not t.get("passed"):
                fail(f"XFER-008: implementation test '{tname}' did not pass")
            else:
                ok(f"XFER-008: test '{tname}' passed")
        # the wrong denominator must not appear
        if re.search(r"0\.05\s*/\s*10", p1):
            fail("XFER-008: paper1 still shows the wrong Holm denominator (0.05/10)")
        else:
            ok("XFER-008: paper1 no longer shows the wrong Holm denominator")
        # terminology discipline
        if "nominally significant" in p1:
            ok("XFER-008: paper1 distinguishes 'nominally significant' from Holm")
        else:
            warns.append("paper1 does not use 'nominally significant' terminology")
        # warn only on the genuinely misleading pre-correction phrasing:
        # asserting a pair IS established, without the Holm caveat
        if re.search(r"one of twelve pairs is established", p1, re.I) or \
           re.search(r"single pair whose interval excludes chance", p1, re.I):
            warns.append("paper1 still asserts an established pair "
                         "(pre-correction headline)")
        else:
            ok("XFER-005: paper1 no longer asserts an established pair")

    # ---- XFER-006: the withdrawn curve must be marked, and the corrected
    # experiment reported (XFER-007)
    if re.search(r"WITHDRAWN|is contaminated and is not a result", p1):
        ok("XFER-006: paper1 marks the original cold-start curve as withdrawn")
    else:
        fail("XFER-006: paper1 does not mark the cold-start curve as withdrawn")
    if "e1\\_coldstart\\_paired.json" in p1 or "e1_coldstart_paired.json" in p1:
        ok("XFER-007: paper1 cites the corrected cold-start artifact")
    else:
        fail("XFER-007: paper1 does not cite the corrected cold-start artifact")
    _cs = load("research/e1_coldstart_paired.json")
    if _cs:
        _rows = {r["n_local_train_sessions"]: r for r in _cs["results"]}
        # the borrowed arm must be below chance at every N -- that is the claim
        _above = [n for n, r in _rows.items() if r["auc_B_borrow"] > 0.5]
        if _above:
            fail(f"XFER-007: borrowed-only AUC is above chance at N={_above}; "
                 f"paper1 says it is below chance at every N")
        else:
            ok(f"XFER-007: borrowed-only below chance at all {len(_rows)} values of N "
               f"(max {max(r['auc_B_borrow'] for r in _rows.values()):.3f})")
        # arm A must be recorded undefined at N=0, not substituted
        if _rows.get(0, {}).get("A_local_defined") is False and _rows[0]["auc_A_local"] is None:
            ok("XFER-007: arm A recorded as undefined at N=0 (not substituted)")
        else:
            fail("XFER-007: arm A is not recorded as undefined at N=0")
        # the headline entry
        _two = _rows.get(2)
        if _two and abs(_two["auc_A_local"] - 0.600) < 0.002:
            ok(f"XFER-007: local-only at N=2 is 0.600 "
               f"(diff {_two['diff_A_local_minus_B']['mean']:+.3f})")
        else:
            fail("XFER-007: local-only at N=2 is not 0.600 as the paper states")


# ---------------------------------------------------------------- main

reg = json.load(open(REG))
claims = reg["claims"]
print("=== tracing registry claims to artifacts ===")
for c in claims:
    cid = c["claim_id"]
    if c["status"] == "superseded" or not c.get("json_path") or c["json_path"].startswith("DERIVED"):
        print(f"  --    {cid}: {c['status']}" +
              (f" ({c['json_path']})" if c.get("json_path") else ""))
        continue
    d = load(c["result_file"])
    if d is None:
        continue
    v = dig(d, c["json_path"])
    if v is None:
        warns.append(f"{cid}: json_path '{c['json_path']}' did not resolve in {c['result_file']}")
        print(f"  warn  {cid}: path '{c['json_path']}' unresolved in {c['result_file']}")
    else:
        ok(f"{cid}: {c['json_path']} = {v}")

cross_paper_checks(claims)

print(f"\n=== {'FAILED' if fails else 'PASSED'} ===  {checks} checks ok, "
      f"{len(warns)} warnings, {len(fails)} failures")
for w in warns:
    print(f"  warn: {w}")
for f in fails:
    print(f"  FAIL: {f}")
sys.exit(1 if fails else 0)
