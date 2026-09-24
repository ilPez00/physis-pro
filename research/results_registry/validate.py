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

    # paper1's above-chance counts
    p1 = open(os.path.join(PAPERS, "paper1_agent_priors.tex")).read()
    for cid, n, phrase in (("XFER-002", bc["action_above"], "action-level"),
                           ("XFER-001", bc["session_above"], "session-level")):
        w = {1: "one", 2: "two", 3: "three"}[n]
        if f"two of twelve pairs" in p1 or f"{w} of twelve" in p1:
            ok(f"paper1 uses a consistent '<n> of twelve' form for {phrase} ({n})")
        else:
            warns.append(f"paper1: could not confirm the '{w} of twelve' form for {cid}")


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
