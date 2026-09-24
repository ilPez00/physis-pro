#!/usr/bin/env python3
"""Verify candidate references for the merged methodology paper.

Prints exact metadata from Crossref so nothing is cited from memory.
"""
import json, subprocess, sys, time

QUERIES = [
    ("king2009robot",
     "The automation of science Robot Scientist King Whelan Jones Reimann Bryant Muggleton Oliver"),
    ("doyle1979tms",
     "A truth maintenance system Doyle Artificial Intelligence 1979"),
    ("agm1985",
     "On the logic of theory change contraction and revision functions Alchourron Gardenfors Makinson"),
    ("chen2024aiscientist",
     "The AI Scientist fully automated open-ended scientific discovery"),
    ("claessen2000quickcheck",
     "QuickCheck lightweight tools for random testing of Haskell programs"),
]

UA = "physis-research/1.0 (mailto:research@example.org)"
out = {}
for key, q in QUERIES:
    url = ("https://api.crossref.org/works?rows=2&select=title,author,container-title,"
           "issued,volume,issue,page,DOI,type&query.bibliographic=" + q.replace(" ", "+"))
    try:
        r = subprocess.run(["curl", "-sS", "-H", f"User-Agent: {UA}", url],
                           capture_output=True, text=True, timeout=45)
        items = json.loads(r.stdout)["message"]["items"]
    except Exception as e:                                  # noqa: BLE001
        print(f"{key}: ERROR {e}")
        continue
    print(f"=== {key} ===")
    if not items:
        print("  no match")
    for it in items[:1]:
        print("  title    :", (it.get("title") or ["?"])[0])
        print("  authors  :", [a.get("family") for a in it.get("author", [])][:5])
        print("  venue    :", (it.get("container-title") or ["?"])[0])
        print("  year     :", it.get("issued", {}).get("date-parts"))
        print("  vol/iss/p:", it.get("volume"), it.get("issue"), it.get("page"))
        print("  doi      :", it.get("DOI"), "| type:", it.get("type"))
        out[key] = it
    print()
    time.sleep(1)

json.dump(out, open("/tmp/refs_verified.json", "w"), indent=2)
print("wrote /tmp/refs_verified.json")
