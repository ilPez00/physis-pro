#!/usr/bin/env python3
"""Latency and output-size census of every navigation-facing MCP tool.

Output size is a real cost: an agent pays for it in context on every call, so a
tool that answers correctly in 29 KB is a tool it will stop reaching for.

Note the server is spawned fresh per run, so the first physis_find is always a
cold index build. A long-lived server pays that once.

Run:  python3 scripts/physis-mcp-bench.py
"""
import subprocess, json, os, time, sys
import pathlib
ROOT=str(pathlib.Path(__file__).resolve().parent.parent)
env=dict(os.environ); env.update({"PHYSIS_ALLOW_DEV_LICENCE":"1",
  "PHYSIS_SCAN_ROOT":ROOT,"PHYSIS_DATA_DIR":ROOT+"/.physis"})
p=subprocess.Popen([ROOT+"/target/release/physis-pro","serve","--stdio"],
  stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
  text=True,env=env,cwd=ROOT)
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
send({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"bench","version":"1"}}})
p.stdout.readline()
send({"jsonrpc":"2.0","method":"notifications/initialized"})
send({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
tools=json.loads(p.stdout.readline())["result"]["tools"]
print("TOOLS:",len(tools))
for t in tools: print("  ",t["name"], len(t.get("description","")))

CALLS=[
 ("physis_health",{}),
 ("physis_stats",{}),
 ("physis_find",{"query":"coherence edit","root":ROOT,"top_k":8}),
 ("physis_pack",{"query":"fix clippy error in coherence edit","root":ROOT,"budget":4000}),
 ("physis_recall",{"query":"coherence edit","top_k":8}),
 ("physis_coherence_query",{"q":"coherence"}),
 ("physis_workflow",{"path":ROOT}),
 ("physis_classify",{"text":"src/content.rs section index"}),
 ("physis_themes",{}),
 ("physis_why",{"path":ROOT+"/src/content.rs"}),
 ("physis_query",{"query":"coherence"}),
 ("physis_route",{"query":"where is the content index built"}),
 ("physis_causes",{"query":"find returns no hits"}),
 ("physis_wiki_search",{"query":"coherence"}),
]
rid=10
res={}
for name,args in CALLS:
    rid+=1; t0=time.time()
    send({"jsonrpc":"2.0","id":rid,"method":"tools/call","params":{"name":name,"arguments":args}})
    line=p.stdout.readline()
    dt=time.time()-t0
    try:
        r=json.loads(line)
        txt=r.get("result",{}).get("content",[{}])[0].get("text","") or json.dumps(r.get("error",""))
    except Exception as e:
        txt="PARSE_FAIL "+str(e)+line[:200]
    res[name]=(dt,len(txt),txt)
    print(f"{name:26s} {dt:7.2f}s {len(txt):7d}B")
print("\n====== SAMPLES ======")
for name,(dt,n,txt) in res.items():
    print(f"\n--- {name} ({dt:.2f}s, {n}B) ---")
    print(txt[:700])
p.kill()
