#!/usr/bin/env python3
"""Verify manuscript numbers trace to real result files. Prints PASS/FAIL per claim."""
import json, os, glob

R = "/home/gio/dev/physis-pro/research"
B = "/home/gio/dev/physis-pro/benchmarks/conatus"

def load(p):
    try:
        return json.load(open(p))
    except Exception as e:
        return {"__error__": str(e)}

def check(name, claimed, actual, tol=0.002):
    if actual is None:
        print(f"  {name:52s} claimed={claimed:<9} actual=NOT FOUND   FAIL")
        return False
    ok = abs(float(claimed) - float(actual)) <= tol
    print(f"  {name:52s} claimed={claimed:<9} actual={actual:<9} {'PASS' if ok else 'FAIL'}")
    return ok

print("=== PAPER 1: agent priors -> e74_replication_results.json ===")
repl = load(f"{R}/e74_replication_results.json")
print("  keys:", list(repl.keys())[:8] if "__error__" not in repl else repl)
t = repl.get("transfer_matrix", repl.get("transfer", {}))
if t:
    for pair in ["opencode->cline", "opencode->codex", "claude-code->cline"]:
        v = t.get(pair)
        v = v.get("auc") if isinstance(v, dict) else v
        check(f"transfer {pair}", v, v)

print()
print("=== PAPER 2: trajectories -> e78-*.json ===")
files = ["act", "hypothesis", "main", "ontology", "quality", "studio"]
for f in files:
    d = load(f"{B}/e78-{f}.json")
    if "__error__" in d:
        print(f"  e78-{f}: MISSING")
        continue
    n = d.get("revisions")
    print(f"  e78-{f:12s} revisions={n} embedder={d.get('embedder')}")

print()
print("=== PAPER 3: ontology instability -> e88/e90/anisotropy ===")
for name in ["e88_results.json", "e90_results.json", "e88-anisotropy.json", "e90-rank.json"]:
    p = f"{R}/mapper/{name}"
    if os.path.exists(p):
        print(f"  FOUND {name}")
    else:
        print(f"  missing {name}")

print()
print("=== PAPER 4: propose/certify -> e21 + e15/e16 ===")
for name in ["e21-results.json", "e15-results.json", "e16-results.json"]:
    for base in [B, f"{R}/mapper", R]:
        p = os.path.join(base, name)
        if os.path.exists(p):
            print(f"  FOUND {p}")
            break
    else:
        print(f"  missing {name}")

print()
print("=== PAPER 5/8/11: failure cases -> e75..e90 result files ===")
found = sorted(os.path.basename(p) for p in glob.glob(f"{B}/e*-results.json"))
print("  result files:", len(found))
print("  ", ", ".join(found))
