#!/usr/bin/env python3
"""Arms A/B/C/D of the 2x2 in RESCOPE.md §3 — does a compiled context let a
small model answer what a big model answers?

                      naive context        physis-packed context
    large model       B (the ceiling)      A (does compression lose it?)
    small model       D (the control)      C (the thesis)

The gate is `must_mention`: the decisive identifiers that a correct answer has
to name, established by reading the code, checked literally and
case-insensitively. Same rule as `oracle::cites_evidence` in physis-core —
no grading model, no partial credit, nothing a participant can argue with.

Both context arms are built to the SAME character budget per case, so the
comparison is context QUALITY at equal size, not size. The naive arm is
round-robin file heads over the whole tree — the shallow dump a developer
actually pastes — not an alphabetical prefix, which would be a strawman that
loses by construction.

Usage:
    python3 benchmarks/retrieval/answer.py --arms C,D [--limit N]

Env (small model):  PHYSIS_SMALL_URL (default http://localhost:11434/v1)
                    PHYSIS_SMALL_MODEL (default qwen3:4b), PHYSIS_SMALL_KEY
Env (large model):  PHYSIS_ORACLE_URL / PHYSIS_ORACLE_MODEL / PHYSIS_ORACLE_KEY
                    (same names oracle.rs reads; also OPENROUTER_API_KEY)

An arm whose model has no endpoint reports `not_configured` and is NOT scored.
A 4B model is a peer, not an oracle: do not point PHYSIS_ORACLE_* at one and
call the result arm B.
"""
import argparse, importlib.util, json, os, pathlib, re, subprocess, sys, time, urllib.request

HERE = pathlib.Path(__file__).resolve().parent
ROOT = HERE.parents[1]

_spec = importlib.util.spec_from_file_location("retrieval_run", HERE / "run.py")
_run = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_run)          # reuse pack(), BIN, ARMS

ARMS = {  # arm: (context, model tier)
    "A": ("packed", "large"), "B": ("naive", "large"),
    "C": ("packed", "small"), "D": ("naive", "small"),
}

PROMPT = (
    "You are answering a question about a Rust codebase using ONLY the context below.\n"
    "Answer in at most 3 sentences. Name the exact identifiers involved.\n"
    "If the context does not contain the answer, say NOT IN CONTEXT.\n\n"
    "--- context ---\n{ctx}\n--- end context ---\n\nQuestion: {q}\nAnswer:"
)


def tier_config(tier):
    """(url, model, key) or None when the tier has no endpoint configured."""
    if tier == "small":
        url = os.environ.get("PHYSIS_SMALL_URL") or "http://localhost:11434/v1"
        model = os.environ.get("PHYSIS_SMALL_MODEL") or "qwen3:4b"
        return url, model, os.environ.get("PHYSIS_SMALL_KEY", "")
    url = os.environ.get("PHYSIS_ORACLE_URL") or "https://openrouter.ai/api/v1"
    model = os.environ.get("PHYSIS_ORACLE_MODEL") or "openai/gpt-4o"
    key = (os.environ.get("PHYSIS_ORACLE_KEY") or os.environ.get("OPENROUTER_API_KEY")
           or os.environ.get("GROQ_API_KEY") or "")
    return (url, model, key) if key else None


def ask(cfg, prompt, timeout=1800):
    """One OpenAI-compatible chat completion. Returns (text, prompt_tokens)."""
    url, model, key = cfg
    body = json.dumps({
        "model": model, "temperature": 0, "stream": False,
        "messages": [{"role": "user", "content": prompt}],
        # qwen3 and friends think by default and neither `think` nor
        # `chat_template_kwargs` turns it off through ollama's OpenAI endpoint
        # (measured). Harmless for the gate: ollama returns the reasoning in a
        # separate `reasoning` field, so only the answer is scored. It is not
        # harmless for the clock — hence the 1800 s timeout.
        "chat_template_kwargs": {"enable_thinking": False},
    }).encode()
    hdr = {"Content-Type": "application/json"}
    if key:
        hdr["Authorization"] = f"Bearer {key}"
    req = urllib.request.Request(url.rstrip("/") + "/chat/completions", body, hdr)
    with urllib.request.urlopen(req, timeout=timeout) as r:
        d = json.loads(r.read())
    text = d["choices"][0]["message"]["content"] or ""
    text = re.sub(r"<think>.*?</think>", "", text, flags=re.S)
    return text.strip(), d.get("usage", {}).get("prompt_tokens")


def pack_context(query, root, budget, top_k, data_dir):
    """The context physis hands an agent: the chunk table (path:lines) plus the
    assembled text. Paths are part of physis's output surface, so the naive arm
    gets path headers too — see naive_context."""
    env = dict(os.environ, PHYSIS_ALLOW_DEV_LICENCE="1", PHYSIS_DATA_DIR=str(data_dir),
               PHYSIS_SCAN_ROOT=str(ROOT))
    env.pop("PHYSIS_EMBEDDER", None)          # semantic cascade, not the hash
    p = subprocess.run([str(_run.BIN), "pack", str(root), query, "--budget", str(budget),
                        "--top-k", str(top_k), "--show-context"],
                       cwd=ROOT, env=env, capture_output=True, text=True, timeout=1800)
    if p.returncode != 0:
        sys.exit(f"pack failed: {p.stderr.strip().splitlines()[-1:]}")
    # Drop the licence/embedder chatter; keep the table and the context body.
    lines = [l for l in p.stdout.splitlines() if not l.startswith(("physis:", "[physis]"))]
    return "\n".join(lines).strip()


def naive_context(root, char_budget):
    """Round-robin heads of every file under root, to char_budget."""
    files = sorted(p for p in (ROOT / root).rglob("*.rs") if p.is_file())
    if not files:
        return ""
    texts = [p.read_text(errors="replace") for p in files]
    per = max(200, char_budget // len(files))
    out, used = [], 0
    for p, t in zip(files, texts):
        head = f"{p.relative_to(ROOT)}:\n{t[:per]}\n"
        if used + len(head) > char_budget:
            head = head[: char_budget - used]
        out.append(head)
        used += len(head)
        if used >= char_budget:
            break
    return "".join(out)


def gate(answer, must):
    low = answer.lower()
    missing = [m for m in must if m.lower() not in low]
    return (not missing), missing


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arms", default="C,D")
    ap.add_argument("--budget", type=int, default=3000)
    ap.add_argument("--top-k", type=int, default=30)
    ap.add_argument("--root", default="physis-core/src")
    ap.add_argument("--data-dir", default=".physis-bench")
    ap.add_argument("--limit", type=int, default=0)
    args = ap.parse_args()

    if not _run.BIN.exists():
        sys.exit(f"missing {_run.BIN}; build it:\n  cargo build --release --features mcp,cli --bin physis-pro")
    cases = json.loads((HERE / "cases.json").read_text())["cases"]
    rootrel = args.root.rstrip("/") + "/"
    scoped = [c for c in cases if all(e.startswith(rootrel) for e in c["expect"])]
    ungated = [c["id"] for c in scoped if not c.get("must_mention")]
    if ungated:
        sys.exit("no must_mention on: " + ", ".join(ungated) + " — a case without a gate cannot be scored")
    if args.limit:
        scoped = scoped[: args.limit]
    print(f"{len(scoped)} cases under {args.root}")

    data_dir = ROOT / args.data_dir
    if not (data_dir / "license.key").exists():
        sys.exit(f"no dev licence at {data_dir/'license.key'}\n  cd physis-dev-license && cargo run -q > {data_dir/'license.key'}")

    arms = [a.strip().upper() for a in args.arms.split(",")]
    for a in arms:
        if a not in ARMS:
            sys.exit(f"unknown arm {a!r}; known: {', '.join(ARMS)}")

    # Pack once per case: shared by every packed arm, and its length sets the
    # naive arm's budget so both sides are compared at equal size.
    packed = {}
    if any(ARMS[a][0] == "packed" for a in arms):
        for c in scoped:
            t0 = time.time()
            packed[c["id"]] = pack_context(c["query"], args.root, args.budget,
                                           args.top_k, data_dir)
            print(f"  packed {c['id']} ({len(packed[c['id']])}c, {time.time()-t0:.1f}s)")
    results = {"root": args.root, "budget": args.budget, "arms": {}}
    for arm in arms:
        ctx_kind, tier = ARMS[arm]
        cfg = tier_config(tier)
        if cfg is None:
            results["arms"][arm] = {"status": "not_configured", "tier": tier,
                                    "reason": "no PHYSIS_ORACLE_KEY / OPENROUTER_API_KEY"}
            print(f"\n=== arm {arm} ({ctx_kind}/{tier}) — not_configured, NOT scored ===")
            continue
        print(f"\n=== arm {arm} ({ctx_kind}/{tier}: {cfg[1]}) ===")
        rows, passes, errors = [], 0, 0
        for c in scoped:
            ctx = packed[c["id"]] if ctx_kind == "packed" else naive_context(args.root, len(packed[c["id"]]) if packed else args.budget * 4)
            t0 = time.time()
            try:
                ans, ptok = ask(cfg, PROMPT.format(ctx=ctx, q=c["query"]))
            except Exception as e:
                errors += 1
                rows.append({"id": c["id"], "pass": None, "error": str(e)[:200]})
                print(f"  {c['id']:<26} ERROR {str(e)[:80]}")
                continue
            ok, missing = gate(ans, c["must_mention"])
            passes += ok
            rows.append({"id": c["id"], "pass": ok, "missing": missing,
                         "context_chars": len(ctx), "prompt_tokens": ptok,
                         "seconds": round(time.time() - t0, 1), "answer": ans})
            print(f"  {c['id']:<26} {'PASS' if ok else 'fail'} "
                  f"missing={missing} ctx={len(ctx)}c tok={ptok} ({time.time()-t0:.1f}s)")
        n = len(scoped) - errors
        # A case that never reached the model is not a failed case. Scoring it
        # as one would understate the arm and hide the outage — the exact shape
        # rule 6 names: an empty result and a check that never ran must not look
        # alike. An arm with any error reports no rate at all.
        st = "ok" if errors == 0 else "incomplete"
        results["arms"][arm] = {"status": st, "context": ctx_kind, "tier": tier,
                                "model": cfg[1], "passes": passes, "scored": n,
                                "errors": errors, "cases": len(scoped),
                                "rate": (passes / n) if (n and not errors) else None,
                                "rows": rows}
        if errors:
            print(f"  -> {arm}: INCOMPLETE — {errors}/{len(scoped)} cases never reached the model; no rate reported")
        else:
            print(f"  -> {arm}: {passes}/{n} = {passes/n:.2f}")

    # One file per arm set: two arm sets running at once against different
    # endpoints would otherwise overwrite each other's results.
    OUT = HERE / ("answer-results-" + "".join(sorted(arms)) + ".json")
    OUT.write_text(json.dumps(results, indent=2))
    print(f"\nwrote {OUT.relative_to(ROOT)}")
    r = {a: v["rate"] for a, v in results["arms"].items()
         if v.get("status") == "ok" and v.get("rate") is not None}
    if "C" in r and "D" in r:
        print(f"C - D = {r['C']-r['D']:+.2f}   (physis contributed this much to the small model)")
    if "C" in r and "B" in r:
        print(f"C - B = {r['C']-r['B']:+.2f}   (the thesis: 0 means the small model matched the ceiling)")
    if "A" in r and "B" in r:
        print(f"A - B = {r['A']-r['B']:+.2f}   (compression loss on the large model)")
    missing = [a for a in "ABCD" if a not in r]
    if missing:
        print("UNSCORED ARMS: " + ", ".join(missing) + " — the 2x2 is incomplete; "
              "no thesis claim may be made from what ran.")


if __name__ == "__main__":
    main()
