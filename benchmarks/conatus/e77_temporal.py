#!/usr/bin/env python3
"""E77 — timestamps, or temporal semantics?

Phases 5 and 6 of the 360 investigation. The claim under test is that Physis
knows a statement can be true at t0 and false at t1, rather than merely
recording when things were written.

World: a machine whose temperature rises.

    t0  60      t1  80      t2  95

Claim: "machine temperature stays below 70".

The claim is true at t0 and false afterwards. Evidence is added in that order,
and after each step the engine is asked to replay the belief state at each
timestamp:

    replay --subject <id> --at t0
    replay --subject <id> --at t1
    replay --subject <id> --at t2

Falsification: if the three replays return the same status once all evidence is
in, then the engine has timestamps and not temporal semantics — belief is a
function of everything known now, not of what was known then.

Control arm: the same evidence in reverse arrival order. If replay at t0 depends
on arrival order rather than on the timestamps carried by the evidence, then the
"temporal" answer is really an insertion-order answer.

    python3 benchmarks/conatus/e77_temporal.py
"""
import argparse, json, os, re, shutil, subprocess, tempfile

UUID = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")
STATUS = re.compile(r"\b(Candidate|Supported|Contradicted|Refuted|Certified|Retired)\b", re.I)

TIMES = ["2026-01-01T00:00:00Z", "2026-06-01T00:00:00Z", "2026-12-01T00:00:00Z"]
EVIDENCE = [
    ("at 2026-01-01 the machine ran at 60 degrees, below the 70 limit", "supporting"),
    ("at 2026-06-01 the machine ran at 80 degrees, above the 70 limit", "contradicting"),
    ("at 2026-12-01 the machine ran at 95 degrees, above the 70 limit", "contradicting"),
]


def run(bin_path, store, args, timeout=180):
    env = dict(os.environ, PHYSIS_CORE_DIR=store, PHYSIS_ALLOW_DEV_LICENCE="1")
    p = subprocess.run([bin_path] + args, capture_output=True, text=True, env=env, timeout=timeout)
    return p.stdout + p.stderr


def state(store, hid):
    data = json.load(open(os.path.join(store, "nodes.json")))
    h = data.get("hypotheses", {}).get(hid, {})
    return h.get("status"), h.get("fitness")


def build(bin_path, tmp, evidence):
    store = tempfile.mkdtemp(dir=tmp)
    out = run(bin_path, store, ["hypothesis", "create", "machine temperature stays below 70"])
    ids = UUID.findall(out)
    if not ids:
        raise SystemExit(f"could not create a hypothesis:\n{out}")
    hid = ids[0]
    for text, polarity in evidence:
        run(bin_path, store, ["hypothesis", "evidence", hid, text, "--polarity", polarity])
    return store, hid


def replays(bin_path, store, hid):
    out = {}
    for when in TIMES:
        text = run(bin_path, store, ["replay", "--subject", hid, "--at", when])
        found = STATUS.search(text)
        out[when] = {
            "status": found.group(1) if found else None,
            "said": " | ".join(line.strip() for line in text.strip().splitlines()[:3]),
        }
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=os.path.expanduser("~/dev/physis-pro/target/release/physis-core"))
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    tmp = tempfile.mkdtemp(prefix="e77-")
    report = {}
    try:
        store, hid = build(args.bin, tmp, EVIDENCE)
        report["status_now"], report["fitness_now"] = state(store, hid)
        report["forward"] = replays(args.bin, store, hid)

        rev_store, rev_hid = build(args.bin, tmp, list(reversed(EVIDENCE)))
        report["reverse_status_now"], _ = state(rev_store, rev_hid)
        report["reverse"] = replays(args.bin, rev_store, rev_hid)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"status after all three observations: {report['status_now']} "
          f"(fitness {report['fitness_now']})")
    print(f"\n{'replayed at':26}{'forward order':>18}{'reverse order':>18}")
    for when in TIMES:
        print(f"{when:26}{report['forward'][when]['status']!s:>18}"
              f"{report['reverse'][when]['status']!s:>18}")
    print("\nwhat replay actually printed (first lines, forward arm):")
    for when in TIMES:
        print(f"  {when}: {report['forward'][when]['said'][:150]}")

    # The question is not whether replay answers differently at different
    # timestamps — it does, because nothing was written before the run. The
    # question is whether it tracks the times the evidence is ABOUT. The
    # evidence says 60 degrees in January and 80 in June; if content time
    # mattered, June would already be Contradicted.
    forward = {w: report["forward"][w]["status"] for w in TIMES}
    june = forward[TIMES[1]]
    print()
    if june == "Contradicted":
        print("VERDICT: belief at 2026-06-01 is already Contradicted, so the engine is reading "
              "the times the evidence is ABOUT — temporal semantics are present.")
    elif june is None:
        print("VERDICT: at 2026-06-01 the engine reports 'no recorded belief', although the "
              "evidence it holds says the machine ran at 80 degrees on that date. Replay is "
              "indexed by WRITE time, not by the time an observation is about: the world times "
              "in the evidence are text. The engine has ingestion timestamps, not temporal "
              "semantics, and a claim that is true at t0 and false at t1 cannot be represented "
              "in this path.")
    else:
        print(f"VERDICT: belief at 2026-06-01 is {june} — read the raw lines above before "
              "concluding anything.")
    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
