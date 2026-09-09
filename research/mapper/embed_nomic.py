#!/usr/bin/env python3
"""Re-embed the mapper corpus with a bigger/newer embedder, for E21.

corpus.json ships bge-base-en-v1.5 768d. The question is whether a stronger
embedder moves the proposer numbers, or whether 57-way filing is limited by
something an embedder cannot supply.
"""
import json, os, sys, urllib.request
HERE = os.path.dirname(os.path.abspath(__file__))
HOST = os.environ.get("E21_EMB_HOST", "http://100.116.39.57:11434")
MODEL = os.environ.get("E21_EMB_MODEL", "nomic-embed-text:latest")
OUT = os.path.join(HERE, "corpus_vecs_%s.json" % MODEL.split(":")[0].replace("-", "_"))

def embed(t):
    body = json.dumps({"model": MODEL, "input": t}).encode()
    req = urllib.request.Request(HOST + "/api/embed", data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as f:
        return json.load(f)["embeddings"][0]

def main():
    entries = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    done = {}
    if os.path.exists(OUT):
        done = json.load(open(OUT))["vecs"]
    for i, e in enumerate(entries):
        if e["name"] in done:
            continue
        # same text the BGE vector was built from: name plus its hints
        text = e["name"] + ": " + ", ".join(e.get("hints") or [])
        done[e["name"]] = embed(text)
        if i % 100 == 0:
            print(f"  {i}/{len(entries)}", file=sys.stderr, flush=True)
            json.dump({"model": MODEL, "vecs": done}, open(OUT, "w"))
    json.dump({"model": MODEL, "vecs": done}, open(OUT, "w"))
    print(f"wrote {len(done)} vectors -> {OUT}", file=sys.stderr)

if __name__ == "__main__":
    main()
