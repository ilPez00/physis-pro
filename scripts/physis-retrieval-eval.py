#!/usr/bin/env python3
"""Retrieval quality gate for physis_pack / physis_find.

Each case is a natural-language question plus the file that must appear in the
top 5. It measures the thing an agent actually needs -- "did the right file come
back" -- which no unit test covers, because ranking quality is a property of the
whole corpus and not of any one function.

Baseline when written (physis-pro, 944 files): pack 4/5, find 3/5.
A drop below that is a regression in retrieval even when every test passes.

Run:  python3 scripts/physis-retrieval-eval.py
Needs: a release build with --features mcp, and a licence in .physis/.
"""
import subprocess, json, os, time, sys
import pathlib
ROOT=str(pathlib.Path(__file__).resolve().parent.parent)
env=dict(os.environ); env.update({"PHYSIS_ALLOW_DEV_LICENCE":"1",
  "PHYSIS_SCAN_ROOT":ROOT,"PHYSIS_DATA_DIR":ROOT+"/.physis"})
p=subprocess.Popen([ROOT+"/target/release/physis-pro","serve","--stdio"],
  stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,env=env,cwd=ROOT)
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"r","version":"1"}}})
p.stdout.readline(); send({"jsonrpc":"2.0","method":"notifications/initialized"})
rid=[10]
def call(name,args):
    rid[0]+=1; t0=time.time()
    send({"jsonrpc":"2.0","id":rid[0],"method":"tools/call","params":{"name":name,"arguments":args}})
    r=json.loads(p.stdout.readline())
    return time.time()-t0, r.get("result",{}).get("content",[{}])[0].get("text","")

# (query, substring that MUST appear in a top-5 path)
CASES=[
 ("where the content index is built and searched","content.rs"),
 ("MCP tool handler registration and dispatch","mcp.rs"),
 ("chunk files into candidates for packing","pack.rs"),
 ("embedding cache keyed by content hash","pack.rs"),
 ("walk directories and ingest documents","ingest.rs"),
]
for tool,argf in [("physis_pack",lambda q:{"query":q,"root":ROOT,"budget":2000}),
                  ("physis_find",lambda q:{"query":q,"root":ROOT,"top_k":5})]:
    hits=0; tot=0.0
    print(f"\n=== {tool} ===")
    for q,want in CASES:
        dt,txt=call(tool,argf(q)); tot+=dt
        paths=[l.split("path:")[1].strip() for l in txt.splitlines() if "path:" in l][:5]
        ok=any(want in x for x in paths)
        hits+=ok
        print(f"  {'HIT ' if ok else 'MISS'} {dt:6.2f}s want={want:12s} got={[x.split('/')[-1] for x in paths][:5]}")
    print(f"  -> {hits}/{len(CASES)} top-5, {tot/len(CASES):.2f}s avg")
p.kill()
