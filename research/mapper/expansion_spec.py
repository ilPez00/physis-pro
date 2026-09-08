#!/usr/bin/env python3
"""Corpus expansion — which cells need entries, how many, and what it buys.

Every experiment in this track converged on the same constraint: 658 entries is
too few for a 70-cell grid. This turns that into a worklist.

  DEFICIT   per cell, against the target E7's F1(n) curve identifies
  FORECAST  what each cell reaches once populated, from the same curve
  CANDIDATES mined from the corpora already embedded -- the codebase and the
            document library -- ranked by proximity to the cell's own members
            and its authored anchor

PRIVACY. The candidate mining reads the document library. It emits candidate
TERMS, never document text and never filenames: an ontology entry is a short
concept name, not an excerpt, so terms are both the right output and the safe
one. The ranked worklist is written to a gitignored file; the committed spec
carries only counts and forecasts.
"""
import json, os, re, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from mapper import l2

HERE = os.path.dirname(os.path.abspath(__file__))
TARGET = 20          # E7: n~20 buys F1 0.26-0.49; the curve flattens after
CURVE = [(3, 0.06), (6, 0.10), (10, 0.21), (20, 0.38), (40, 0.46), (10**9, 0.52)]


def forecast(n):
    for lim, f in CURVE:
        if n <= lim:
            return f
    return 0.52


# The document library is substantially Italian; an English-only stop list let
# "della/delle/degli/nella" dominate every mode's candidate list on the first
# run. Both languages, plus the repo's own identifiers.
STOP = set("""the a an and or of to in for with on at by from as is are was were be been
being this that these those it its his her their our your my we you they he she
not no nor but if then than so such can may might will would shall should must
one two three all any some each other more most much many few less least own same
very just also into over under between within without about after before during
while when where which who whom whose what how why do does did done have has had
using used use new non per via etc eg ie vs cf ibid op cit
della delle degli dello dei del alla alle allo agli nella nelle nello negli sulla
sulle sullo con per tra fra come anche questo questa questi queste quello quella
essere sono stato stata stati state hanno aveva avere fare cosa dove quando perche
piu meno molto tutto tutti tutte ogni altro altra altri altre stesso stessa senza
sopra sotto dopo prima mentre quindi allora infatti ossia cioe ovvero
physis physis-pro assert unwrap struct impl pub fn let mut const enum trait crate
todo fixme lorem ipsum menu-item test tests string vec self none some""".split())


def terms(text):
    toks = re.findall(r"[A-Za-z][A-Za-z\-']{3,}", text.lower())
    out = []
    for i, t in enumerate(toks):
        if t in STOP or len(t) < 4:
            continue
        out.append(t)
        if i + 1 < len(toks) and toks[i + 1] not in STOP and len(toks[i + 1]) >= 4:
            out.append(t + " " + toks[i + 1])
    return out


def main():
    E = json.load(open(os.path.join(HERE, "corpus.json")))["entries"]
    n = len(E)
    X = l2(np.array([e["vec"] for e in E], dtype=np.float32))
    cell = [f"{e['domain']}/{e['mode']}" for e in E]
    mode = [e["mode"] for e in E]
    A = json.load(open(os.path.join(HERE, "anchors.json")))["anchors"]
    AV = l2(np.array([a["vec"] for a in A], dtype=np.float32))
    akey = [f"{a['domain']}/{a['mode']}" for a in A]

    print("Corpus expansion spec\n")
    print(f"  current: {n} entries across {len(set(cell))} occupied cells of 70")

    # ── deficits ────────────────────────────────────────────────────────
    sizes = Counter(cell)
    msizes = Counter(mode)
    all_cells = sorted(set(akey))
    deficit = {c: max(0, TARGET - sizes.get(c, 0)) for c in all_cells}
    need = sum(deficit.values())
    print(f"  target : {TARGET} entries per cell (E7's curve flattens after ~20)")
    print(f"  deficit: {need} entries across {sum(1 for v in deficit.values() if v)} cells")
    print(f"  that is {n} -> {n + need} entries, a {(n+need)/n:.1f}x expansion\n")

    print("  ── by MODE, worst first ──────────────────────────────────────")
    print(f"  {'mode':<12}{'entries':>9}{'F1 now':>9}{'need':>7}{'F1 at 20':>10}{'gain':>8}")
    rows = []
    for m in sorted(msizes, key=lambda k: msizes[k]):
        cells_m = [c for c in all_cells if c.endswith("/" + m)]
        d = sum(deficit[c] for c in cells_m)
        now, then = forecast(msizes[m] * 0.7), forecast(TARGET * 0.7 * len(cells_m))
        rows.append((m, msizes[m], d))
        print(f"  {m:<12}{msizes[m]:>9}{now:>9.2f}{d:>7}{then:>10.2f}{then-now:>+8.2f}")

    starved = [m for m, k, d in rows if k < 30]
    print(f"\n  the five starved modes hold {sum(k for m,k,d in rows if m in starved)} "
          f"entries between them and need {sum(d for m,k,d in rows if m in starved)}")

    # ── candidate mining ────────────────────────────────────────────────
    print("\n  ── candidate terms, mined from the already-embedded corpora ──")
    pools = []
    for meta, vecs, tag in [("selfcorpus.json", "selfcorpus_vecs.f32", "code"),
                            ("docs_corpus.json", "docs_vecs.f32", "docs")]:
        mp, vp = os.path.join(HERE, meta), os.path.join(HERE, vecs)
        if not (os.path.exists(mp) and os.path.exists(vp)):
            continue
        md = json.load(open(mp))["chunks"]
        V = l2(np.fromfile(vp, dtype=np.float32).reshape(len(md), -1))
        pools.append((tag, md, V))
    if not pools:
        print("    (no mined corpora present — run the exporters first)")
        return

    # text for the doc pool comes from the gitignored working file
    dtext = {}
    dj = os.path.join(HERE, ".docs_chunks.jsonl")
    if os.path.exists(dj):
        raw = [json.loads(l) for l in open(dj)]
        for tag, md, V in pools:
            if tag == "docs" and len(raw) >= len(md):
                step = len(raw) // len(md) + 1 if len(raw) != len(md) else 1
                sel = raw[::step][: len(md)] if step > 1 else raw[: len(md)]
                dtext = {i: sel[i]["text"] for i in range(len(sel))}

    existing = set()
    for e in E:
        existing.update(terms(e["name"]))

    # background document frequency across the WHOLE mined corpus, so a term's
    # association with one mode can be measured against its association overall
    background, total_chunks = Counter(), 0
    for tag, md, V in [p_ for p_ in pools if p_[0] == "docs"]:
        for j in range(len(md)):
            txt = md[j].get("text") or dtext.get(int(j), "")
            if not txt:
                continue
            total_chunks += 1
            for t in set(terms(txt)):
                background[t] += 1
    print(f"    background: {total_chunks} chunks, {len(background)} distinct terms")

    worklist = {}
    for m, k, d in rows:
        if k >= 30 or d == 0:
            continue
        ai = [i for i, a in enumerate(akey) if a.endswith("/" + m)]
        if not ai:
            continue
        target_vec = l2(AV[ai].mean(0, keepdims=True))
        members = [i for i in range(n) if mode[i] == m]
        if members:
            target_vec = l2((AV[ai].mean(0) + X[members].mean(0))[None, :])
        # CONTRASTIVE scoring. The first version summed similarity over the
        # nearest chunks, which just rewards frequency -- every mode came back
        # with the same list of common words. A term earns a place only if it is
        # MORE associated with this mode than with the others, which is the same
        # contrast control this track applies to everything else.
        # DOCS ONLY. Mining physis's own source for physis's ontology entries is
        # circular -- it proposes the system's own vocabulary back at itself, and
        # the second run did exactly that ("ontologyloader rand", "seedablerng",
        # "commit branch"). The document library is genuinely external content.
        cand = Counter()
        for tag, md, V in [p_ for p_ in pools if p_[0] == "docs"]:
            sims = (V @ target_vec.T).ravel()
            for j in np.argsort(-sims)[:150]:
                txt = md[j].get("text") or dtext.get(int(j), "")
                for t in set(terms(txt)):
                    if (t not in existing and len(t) >= 5
                            and not any(ch.isdigit() for ch in t)
                            and all(any(v in w for v in "aeiou") for w in t.split())):
                        cand[t] += 1
        scored = []
        for t, c in cand.items():
            if c < 3:                      # singletons are noise
                continue
            bg = background.get(t, 0)
            if bg > 0.35 * total_chunks:   # appears everywhere: not distinctive
                continue
            lift = (c / max(sum(cand.values()), 1)) / ((bg + 1) / (total_chunks + 1))
            scored.append((lift * np.log1p(c), t))
        scored.sort(reverse=True)
        worklist[m] = [t for _, t in scored[:25]]
        print(f"\n    {m}  (has {k}, needs {d})")
        print(f"      {', '.join(worklist[m][:12])}")

    out = os.path.join(HERE, ".expansion_candidates.json")
    json.dump({"target_per_cell": TARGET, "worklist": worklist}, open(out, "w"), indent=1)
    print(f"\n  full ranked candidates -> {out}  (gitignored)")

    spec = {
        "generated": "expansion_spec.py",
        "current_entries": n,
        "target_per_cell": TARGET,
        "total_deficit": need,
        "by_mode": [{"mode": m, "entries": k, "deficit": d} for m, k, d in rows],
        "forecast_note": "per-mode F1 from E7's measured curve; n~20 -> 0.26-0.49",
    }
    json.dump(spec, open(os.path.join(HERE, "expansion_spec.json"), "w"), indent=1)
    print(f"  committable spec (counts only)   -> expansion_spec.json")


if __name__ == "__main__":
    main()
