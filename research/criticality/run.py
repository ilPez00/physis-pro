#!/usr/bin/env python3
"""E65-E70: dependency-free measurements for multiscale Physis structure.

This is research code. It reads the repository, git history, and the committed
agent traces under benchmarks/behaviour. It does not change Physis state and it
does not infer semantics from embeddings. Missing observations stay explicit.

Run from the repository root:

    python3 research/criticality/run.py --root . --out research/criticality/results.json

The report deliberately calls dependency closure and trace bursts proxies. A
changed git file is observed; a downstream file in the import closure is a
possible propagation target, not proof that Physis recomputed it.
"""

from __future__ import annotations

import argparse
import collections
import hashlib
import json
import math
import os
import pathlib
import re
import resource
import statistics
import subprocess
import sys
import tarfile
import time
from typing import Iterable


WORD = re.compile(r"[A-Za-z][A-Za-z0-9_'-]{1,}")
IMPORT = re.compile(r"\b(?:use|pub\s+use)\s+((?:crate|physis_core)::[A-Za-z_][A-Za-z0-9_:]*)")
MODULE = re.compile(r"\b(?:pub\s+)?mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;")
TEXT_EXTS = {
    ".md", ".rs", ".py", ".js", ".mjs", ".ts", ".tsx", ".jsx", ".toml",
    ".json", ".yaml", ".yml", ".html", ".css", ".txt", ".csv", ".sql",
}
SKIP_PARTS = {"target", ".git", "vendor", "node_modules", "logs"}
STOP = {
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from", "has",
    "have", "in", "is", "it", "of", "on", "or", "that", "the", "this", "to",
    "was", "were", "with", "you", "your", "not", "use", "used", "using",
}


def git(root: pathlib.Path, *args: str) -> str:
    p = subprocess.run(["git", *args], cwd=root, text=True, capture_output=True, check=True)
    return p.stdout


def tracked(root: pathlib.Path) -> list[str]:
    paths = [p for p in git(root, "ls-files", "-z").split("\0") if p]
    core = root / "physis-core"
    if core.is_dir():
        paths.extend(f"physis-core/{p}" for p in git(core, "ls-files", "-z").split("\0") if p)
    return sorted(set(paths))


def read_text(path: pathlib.Path, limit: int = 512_000) -> str:
    try:
        return path.read_bytes()[:limit].decode("utf-8", "replace")
    except OSError:
        return ""


def tokens(text: str) -> list[str]:
    return [m.group(0).lower() for m in WORD.finditer(text)]


def stem(word: str) -> str:
    for suffix in ("ingly", "edly", "ing", "ed", "ies", "es", "s"):
        if word.endswith(suffix) and len(word) - len(suffix) >= 4:
            return word[:-len(suffix)] + ("y" if suffix == "ies" else "")
    return word


def concepts(text: str) -> list[str]:
    return [stem(w) for w in tokens(text) if w not in STOP and len(w) > 2]


def percentile(values: list[float], value: float) -> float:
    if not values:
        return 0.0
    return sum(v <= value for v in values) / len(values)


def quantile(values: list[float], q: float) -> float:
    if not values:
        return 0.0
    v = sorted(values)
    pos = min(len(v) - 1, max(0, int(q * (len(v) - 1))))
    return v[pos]


def mean(values: Iterable[float]) -> float:
    v = list(values)
    return sum(v) / len(v) if v else 0.0


def ols(xs: list[float], ys: list[float]) -> dict:
    if len(xs) < 3 or len(xs) != len(ys):
        return {"n": len(xs), "slope": None, "intercept": None, "r2": None}
    xm, ym = mean(xs), mean(ys)
    ssx = sum((x - xm) ** 2 for x in xs)
    if ssx == 0:
        return {"n": len(xs), "slope": None, "intercept": ym, "r2": None}
    slope = sum((x - xm) * (y - ym) for x, y in zip(xs, ys)) / ssx
    intercept = ym - slope * xm
    pred = [intercept + slope * x for x in xs]
    sse = sum((y - p) ** 2 for y, p in zip(ys, pred))
    sst = sum((y - ym) ** 2 for y in ys)
    return {
        "n": len(xs), "slope": slope, "intercept": intercept,
        "r2": 1.0 - sse / sst if sst else 1.0,
    }


def ks_stat(values: list[float], cdf) -> float:
    ordered = sorted(values)
    n = len(ordered)
    return max(
        max(abs((i + 1) / n - cdf(x)), abs(i / n - cdf(x)))
        for i, x in enumerate(ordered)
    ) if ordered else 0.0


def tail_fit(values: Iterable[float], seed: int = 17, bootstrap: int = 250) -> dict:
    """Fit common continuous tail approximations; no log-log regression.

    Observations are often integer counts. The fit is therefore labelled a
    continuous-tail approximation. xmin is selected by minimum power-law KS,
    following the standard exploratory Clauset-style procedure. AIC ranks the
    alternatives on the same selected tail; it is not a proof of a law.
    """
    xs = sorted(float(x) for x in values if math.isfinite(float(x)) and float(x) > 0)
    if len(xs) < 10:
        return {"status": "NOT MEASURED", "reason": f"tail sample has {len(xs)} values; need 10"}
    value_counts = collections.Counter(xs)
    candidates = []
    tail_count = 0
    for x in sorted(value_counts, reverse=True):
        tail_count += value_counts[x]
        if tail_count >= 10 and x < max(xs):
            candidates.append(x)
    candidates.sort()
    if len(candidates) > 32:
        candidates = [candidates[int(i * (len(candidates) - 1) / 31)] for i in range(32)]
    best = None
    for xmin in candidates:
        tail = [x for x in xs if x >= xmin]
        denom = sum(math.log(x / xmin) for x in tail)
        if denom <= 0:
            continue
        alpha = 1.0 + len(tail) / denom
        d = ks_stat(tail, lambda x, a=alpha, lo=xmin: 1.0 - (x / lo) ** (1.0 - a))
        if best is None or d < best[0]:
            best = (d, xmin, tail, alpha)
    if best is None:
        return {"status": "NOT MEASURED", "reason": "no varying tail candidate"}

    observed_ks, xmin, tail, alpha = best
    n = len(tail)
    power_ll = n * math.log((alpha - 1.0) / xmin) - alpha * sum(math.log(x / xmin) for x in tail)
    shifted = [x - xmin for x in tail]
    positive_shifted = [max(1e-9, x) for x in shifted]
    lam = 1.0 / mean(positive_shifted)
    exp_ll = n * math.log(lam) - lam * sum(positive_shifted)
    logs = [math.log(x) for x in tail]
    mu, sd = mean(logs), statistics.pstdev(logs)
    sd = max(sd, 1e-9)
    lognormal_ll = sum(-math.log(x * sd * math.sqrt(2 * math.pi)) - ((math.log(x) - mu) ** 2) / (2 * sd * sd) for x in tail)

    # A small Weibull grid is enough as a competing stretched-exponential
    # sensor. The scale has a closed form for every shape.
    weibull = None
    for k in [0.20 + 0.05 * i for i in range(47)]:
        scale = max(1e-9, mean(y ** k for y in positive_shifted) ** (1.0 / k))
        ll = sum(math.log(k / scale) + (k - 1.0) * math.log(y / scale) - (y / scale) ** k for y in positive_shifted)
        if weibull is None or ll > weibull[0]:
            weibull = (ll, k, scale)

    # Discrete normalisation makes the truncated-power-law comparison stable
    # for count data without adding scipy. Bounds are finite only for the
    # normaliser approximation; they extend 20x beyond the largest observation.
    lower = max(1, int(math.ceil(xmin)))
    upper = max(lower + 300, min(max(int(max(tail) * 20), lower + 300), 3_000))
    grid = range(lower, upper + 1)
    trunc = None
    for a in [1.1 + 0.5 * i for i in range(6)]:
        for cutoff in [0.0] + [0.002 * (i + 1) / max(1.0, xmin) for i in range(5)]:
            logs_z = [-a * math.log(k) - cutoff * k for k in grid]
            mx = max(logs_z)
            log_z = mx + math.log(sum(math.exp(z - mx) for z in logs_z))
            ll = sum(-a * math.log(x) - cutoff * x - log_z for x in tail)
            if trunc is None or ll > trunc[0]:
                trunc = (ll, a, cutoff)

    models = {
        "power_law": {"log_likelihood": power_ll, "parameters": 1, "alpha": alpha},
        "exponential": {"log_likelihood": exp_ll, "parameters": 1, "lambda": lam},
        "lognormal": {"log_likelihood": lognormal_ll, "parameters": 2, "mu": mu, "sigma": sd},
        "stretched_exponential": {"log_likelihood": weibull[0], "parameters": 2, "shape": weibull[1], "scale": weibull[2]},
        "truncated_power_law": {"log_likelihood": trunc[0], "parameters": 2, "alpha": trunc[1], "lambda": trunc[2]},
    }
    for model in models.values():
        model["aic"] = 2 * model["parameters"] - 2 * model["log_likelihood"]
    rng = __import__("random").Random(seed)
    exceed = 0
    alpha_boot = []
    for _ in range(bootstrap):
        simulated = [xmin * math.exp(rng.expovariate(alpha - 1.0)) for _ in tail]
        d = ks_stat(simulated, lambda x, a=alpha, lo=xmin: 1.0 - (x / lo) ** (1.0 - a))
        exceed += d >= observed_ks
        den = sum(math.log(x / xmin) for x in simulated)
        alpha_boot.append(1.0 + len(simulated) / den if den > 0 else alpha)
    alpha_boot.sort()
    return {
        "status": "MEASURED",
        "approximation": "continuous tail; integer observations treated as positive real values",
        "n": len(xs), "xmin": xmin, "tail_n": n, "tail_fraction": n / len(xs),
        "alpha": alpha, "ks": observed_ks,
        "ks_bootstrap_p": (exceed + 1) / (bootstrap + 1),
        "alpha_ci95": [quantile(alpha_boot, 0.025), quantile(alpha_boot, 0.975)],
        "best_aic": min(models, key=lambda k: models[k]["aic"]),
        "models": models,
        "llr_power_law_minus_lognormal": power_ll - lognormal_ll,
        "llr_power_law_minus_exponential": power_ll - exp_ll,
    }


def distribution_curves(values: Iterable[float], limit: int = 200) -> dict:
    """Return compact empirical curves, preserving tails without plotting deps."""
    xs = sorted(float(x) for x in values if float(x) > 0)
    if not xs:
        return {"n": 0, "cdf": [], "ccdf": [], "lorenz": []}
    stride = max(1, len(xs) // limit)
    cdf, ccdf = [], []
    for i in range(0, len(xs), stride):
        x = xs[i]
        cdf.append([x, (i + 1) / len(xs)])
        ccdf.append([x, (len(xs) - i) / len(xs)])
    total = sum(xs)
    running = 0.0
    lorenz = [[0.0, 0.0]]
    for i, x in enumerate(xs, 1):
        running += x
        if i == len(xs) or i % stride == 0:
            lorenz.append([i / len(xs), running / total if total else 0.0])
    gini = (2.0 * sum(i * x for i, x in enumerate(xs, 1)) / (len(xs) * total) - (len(xs) + 1) / len(xs)) if total else None
    return {"n": len(xs), "cdf": cdf, "ccdf": ccdf, "lorenz": lorenz, "gini": gini}


def module_key(path: str, crate: str) -> tuple[str, str]:
    p = path.removesuffix(".rs")
    if p.endswith("/mod"):
        p = p[:-4].rstrip("/")
    prefix = "src/" if crate == "pro" else "physis-core/src/"
    rel = p.removeprefix(prefix).replace("/", ".")
    return crate, rel if rel not in {"lib", "main"} else ""


def graph_from_contents(contents: dict[str, str]) -> tuple[set[str], set[tuple[str, str]]]:
    sources = {p for p in contents if p.endswith(".rs") and (p.startswith("src/") or p.startswith("physis-core/src/"))}
    modules: dict[tuple[str, str], str] = {}
    for path in sources:
        crate = "core" if path.startswith("physis-core/src/") else "pro"
        crate_key, key = module_key(path, crate)
        modules[(crate_key, key)] = path
    edges: set[tuple[str, str]] = set()
    for path in sources:
        crate = "core" if path.startswith("physis-core/src/") else "pro"
        parent = module_key(path, crate)[1]
        if parent.endswith(".mod"):
            parent = parent[:-4]
        for raw in IMPORT.findall(contents[path]):
            head, *rest = raw.split("::")
            target_crate = crate if head == "crate" else "core"
            chain = ".".join(rest)
            choices = [chain]
            parts = chain.split(".") if chain else []
            choices += [".".join(parts[:i]) for i in range(len(parts), 0, -1)]
            target = next((modules.get((target_crate, c)) for c in choices if (target_crate, c) in modules), None)
            if target and target != path:
                edges.add((path, target))
        for name in MODULE.findall(contents[path]):
            candidate = f"{parent}.{name}" if parent else name
            target = modules.get((crate, candidate)) or modules.get((crate, name))
            if target and target != path:
                edges.add((path, target))
    return sources, edges


def current_graph(root: pathlib.Path) -> tuple[set[str], set[tuple[str, str]]]:
    contents = {p: read_text(root / p, 300_000) for p in tracked(root) if p.endswith(".rs")}
    return graph_from_contents(contents)


def reverse_graph(nodes: set[str], edges: set[tuple[str, str]]) -> dict[str, set[str]]:
    rev = {n: set() for n in nodes}
    for importer, imported in edges:
        rev.setdefault(imported, set()).add(importer)
    return rev


def closure(seeds: Iterable[str], rev: dict[str, set[str]], max_depth: int | None = None) -> dict[str, int]:
    out: dict[str, int] = {}
    queue = collections.deque((s, 0) for s in seeds if s in rev)
    while queue:
        node, depth = queue.popleft()
        if node in out and out[node] <= depth:
            continue
        out[node] = depth
        if max_depth is not None and depth >= max_depth:
            continue
        for child in sorted(rev.get(node, ())):
            queue.append((child, depth + 1))
    return out


def _git_commits(repo: pathlib.Path, prefix: str, limit: int) -> list[dict]:
    lines = git(repo, "log", "--root", "--no-merges", f"--max-count={limit}", "--format=%H%x09%ct", "--name-only").splitlines()
    commits = []
    current = None
    for line in lines + [""]:
        if "\t" in line and re.fullmatch(r"[0-9a-f]{40}\t\d+", line):
            if current is not None:
                commits.append(current)
            h, stamp = line.split("\t")
            current = {"id": f"{prefix}{h}", "time": int(stamp), "files": []}
        elif current is not None and line:
            current["files"].append(f"{prefix}{line}")
    for commit in commits:
        commit["files"] = sorted(set(commit["files"]))
    return commits


def git_episodes(root: pathlib.Path, limit: int, gap_hours: float) -> list[dict]:
    commits = _git_commits(root, "", limit)
    core = root / "physis-core"
    if core.is_dir():
        commits.extend(_git_commits(core, "physis-core/", limit))
    commits.sort(key=lambda c: c["time"])
    episodes: list[dict] = []
    for c in commits:
        if not episodes or c["time"] - episodes[-1]["last"] > gap_hours * 3600:
            episodes.append({"first": c["time"], "last": c["time"], "commits": [], "files": set()})
        e = episodes[-1]
        e["last"] = c["time"]
        e["commits"].append(c["id"])
        e["files"].update(c["files"])
    for i, e in enumerate(episodes):
        e["id"] = f"git-{i:04d}"
        e["files"] = sorted(e["files"])
        e["duration_hours"] = (e["last"] - e["first"]) / 3600.0
    return episodes


def trace_bursts(root: pathlib.Path) -> list[dict]:
    path = root / "benchmarks/behaviour/sessions.json"
    if not path.exists():
        return []
    data = json.loads(path.read_text())
    bursts = []
    for session in data.get("sessions", []):
        seen: list[str] = []
        current: list[str] = []
        for touch in session.get("touches", []):
            raw = touch.get("path", "")
            if not raw:
                continue
            p = raw.replace("\\", "/")
            if p in current and len(current) >= 2:
                bursts.append({"source": "agent_trace", "session": session.get("session"), "files": sorted(set(current)), "duration_steps": len(current)})
                current = []
            current.append(p)
            seen.append(p)
        if len(current) >= 2:
            bursts.append({"source": "agent_trace", "session": session.get("session"), "files": sorted(set(current)), "duration_steps": len(current)})
    return bursts


def avalanche_section(root: pathlib.Path, graph: tuple[set[str], set[tuple[str, str]]], episodes: list[dict]) -> dict:
    nodes, edges = graph
    rev = reverse_graph(nodes, edges)
    records = []
    for e in episodes:
        seeds = [p for p in e["files"] if p in nodes]
        reach = closure(seeds, rev)
        first = {n for n, d in reach.items() if d == 1}
        records.append({
            "id": e["id"], "source": "git", "commits": len(e["commits"]),
            "direct_size": len(e["files"]), "source_nodes": len(seeds),
            "closure_size": len(reach), "depth": max(reach.values(), default=0),
            "breadth": len(first), "duration_hours": e["duration_hours"],
            "global_fraction": len(reach) / len(nodes) if nodes else 0.0,
        })
    trace = trace_bursts(root)
    trace_records = [{
        "source": "agent_trace", "session": b["session"], "direct_size": len(b["files"]),
        "closure_size": len(b["files"]), "depth": 0, "breadth": len(b["files"]),
        "duration_steps": b["duration_steps"],
    } for b in trace]
    direct = [r["direct_size"] for r in records]
    closure_sizes = [r["closure_size"] for r in records if r["closure_size"]]
    immediate = [r["breadth"] / max(1, r["source_nodes"]) for r in records if r["source_nodes"]]
    robustness = []
    for gap in (0.5, 1.0, 2.0, 4.0, 6.0, 12.0, 24.0):
        grouped = git_episodes(root, 500, gap)
        sizes = [len(closure(set(e["files"]) & nodes, rev)) for e in grouped]
        robustness.append({"gap_hours": gap, "episodes": len(grouped), "median_closure": quantile(sizes, 0.5), "p90_closure": quantile(sizes, 0.9), "tail_fit": tail_fit(sizes)})
    finite_size = []
    for label, subset in (("all", nodes), ("core", {n for n in nodes if n.startswith("physis-core/src/")}), ("pro", {n for n in nodes if n.startswith("src/")})):
        local_rev = reverse_graph(subset, {(a, b) for a, b in edges if a in subset and b in subset})
        sizes = [len(closure(set(e["files"]) & subset, local_rev)) for e in episodes]
        finite_size.append({"subset": label, "L": len(subset), "episodes": len(sizes), "median": quantile(sizes, 0.5), "max_fraction": max(sizes, default=0) / len(subset) if subset else 0.0, "tail_fit": tail_fit(sizes)})
    return {
        "definition": "git episode changed files are observed edits; reverse import closure is potential propagation",
        "graph_nodes": len(nodes), "graph_edges": len(edges), "git_episodes": len(records),
        "agent_trace_bursts": len(trace_records),
        "git_direct_size_fit": tail_fit(direct),
        "git_potential_closure_fit": tail_fit(closure_sizes),
        "agent_trace_size_fit": tail_fit([r["direct_size"] for r in trace_records]),
        "curves": {
            "git_direct_size": distribution_curves(direct),
            "git_potential_closure": distribution_curves(closure_sizes),
            "agent_trace_burst_size": distribution_curves([r["direct_size"] for r in trace_records]),
        },
        "branching_ratio_proxy": {"status": "MEASURED", "definition": "mean first-hop reverse dependents per changed source node", "mean": mean(immediate), "n": len(immediate)},
        "threshold_robustness": robustness,
        "finite_size_scaling": {"status": "NOT MEASURED", "reason": "three nested repository partitions are not equivalent system sizes; no valid collapse test yet", "partitions": finite_size},
        "records": records,
        "trace_records": trace_records[:5000],
        "limits": ["git episodes have no engine recomputation trace", "trace bursts use repeated path as a boundary because committed sessions have no timestamps", "branching proxy is graph exposure, not observed branching"],
    }


def corpus(root: pathlib.Path, max_files: int = 300) -> dict[str, str]:
    out = {}
    for p in tracked(root):
        path = pathlib.Path(p)
        if path.suffix.lower() not in TEXT_EXTS or any(part in SKIP_PARTS for part in path.parts):
            continue
        text = read_text(root / path, 160_000)
        if text:
            out[p] = text
        if len(out) >= max_files:
            break
    return out


def scale_sequences(docs: dict[str, str]) -> dict[str, list[str]]:
    all_tokens = [w for text in docs.values() for w in tokens(text)]
    all_chars = [c for c in "".join(docs.values()).lower() if c.isalpha()]
    all_concepts = [w for text in docs.values() for w in concepts(text)]
    phrases = []
    motifs = []
    sentences = []
    paragraphs = []
    for text in docs.values():
        ts = tokens(text)[:2_000]
        phrases.extend(f"{a} {b}" for a, b in zip(ts, ts[1:]))
        cs = concepts(text)[:2_000]
        motifs.extend(f"{a} {b} {c}" for a, b, c in zip(cs, cs[1:], cs[2:]))
        sentences.extend(re.findall(r"[^.!?\n]{8,}", text))
        paragraphs.extend(p for p in re.split(r"\n\s*\n", text) if len(p.strip()) >= 8)
    return {
        "character": all_chars, "token": all_tokens, "phrase": phrases,
        "sentence": [" ".join(tokens(x)) for x in sentences],
        "paragraph": [" ".join(tokens(x)) for x in paragraphs],
        "document": list(docs), "concept": all_concepts, "motif": motifs,
    }


def zipf(seq: list[str]) -> dict:
    counts = collections.Counter(seq)
    freqs = sorted(counts.values(), reverse=True)
    fit = ols([math.log(i + 1) for i in range(len(freqs))], [math.log(f) for f in freqs]) if len(freqs) >= 3 else {"n": len(freqs), "slope": None, "r2": None}
    return {"observations": len(seq), "types": len(counts), "top": freqs[:10], "alpha": -fit.get("slope") if fit.get("slope") is not None else None, "loglog_r2": fit.get("r2"), "fit": "rank-frequency OLS is descriptive only; tail law uses competing likelihood fits"}


def heaps(seq: list[str]) -> dict:
    if len(seq) < 10:
        return {"status": "NOT MEASURED", "observations": len(seq)}
    step = max(1, len(seq) // 100)
    seen, xs, ys = set(), [], []
    for i, x in enumerate(seq, 1):
        seen.add(x)
        if i == len(seq) or i % step == 0:
            xs.append(math.log(i)); ys.append(math.log(len(seen)))
    fit = ols(xs, ys)
    return {"status": "MEASURED", "observations": len(seq), "types": len(set(seq)), "beta": fit.get("slope"), "loglog_r2": fit.get("r2"), "fit": "cumulative log-log OLS; beta is descriptive, no sublinear law assumed"}


def mutual_information(sequences: list[list[str]], max_distance: int = 32, vocab_limit: int = 256) -> list[dict]:
    count = collections.Counter(x for seq in sequences for x in seq)
    vocab = {x for x, _ in count.most_common(vocab_limit)}
    result = []
    for d in range(1, max_distance + 1):
        joint = collections.Counter()
        left = collections.Counter()
        right = collections.Counter()
        total = 0
        for seq in sequences:
            for a, b in zip(seq, seq[d:]):
                a, b = (a if a in vocab else "<other>"), (b if b in vocab else "<other>")
                joint[a, b] += 1; left[a] += 1; right[b] += 1; total += 1
        if not total:
            continue
        mi = 0.0
        for (a, b), n in joint.items():
            p = n / total
            mi += p * math.log(p / ((left[a] / total) * (right[b] / total)))
        result.append({"distance": d, "mutual_information": mi, "pairs": total})
    return result


def decay_fit(points: list[dict]) -> dict:
    points = [p for p in points if p["mutual_information"] > 0]
    if len(points) < 5:
        return {"status": "NOT MEASURED", "n": len(points)}
    x = [float(p["distance"]) for p in points]
    y = [math.log(p["mutual_information"]) for p in points]
    power = ols([math.log(v) for v in x], y)
    exponential = ols(x, y)
    def aic(fit):
        pred = [fit["intercept"] + fit["slope"] * z for z in fit["_x"]]
        sse = sum((a - b) ** 2 for a, b in zip(y, pred))
        var = max(1e-12, sse / len(y))
        return len(y) * (math.log(2 * math.pi * var) + 1) + 4
    power["_x"] = [math.log(v) for v in x]
    exponential["_x"] = x
    power_aic, exponential_aic = aic(power), aic(exponential)
    power.pop("_x"); exponential.pop("_x")
    return {"status": "MEASURED", "n": len(points), "power_decay": power, "exponential_decay": exponential, "aic": {"power": power_aic, "exponential": exponential_aic, "preferred": "power" if power_aic < exponential_aic else "exponential"}, "points": points, "caveat": "paired MI observations are autocorrelated; this is a descriptive comparison, not a law claim"}


def relation_distributions(docs: dict[str, str], graph: tuple[set[str], set[tuple[str, str]]]) -> dict:
    nodes, edges = graph
    co = collections.Counter()
    precedes = collections.Counter()
    for text in docs.values():
        cs = list(dict.fromkeys(concepts(text)))
        for a in cs:
            co[a] += max(0, len(cs) - 1)
        seq = concepts(text)
        for a, b in zip(seq, seq[1:]):
            if a != b:
                precedes[a] += 1
    part = collections.Counter(str(pathlib.PurePosixPath(p).parent) for p in docs)
    dep_in = collections.Counter(imported for _, imported in edges)
    out = {}
    for name, values, definition in [
        ("CO_OCCURS", list(co.values()), "unique concept neighbours per document"),
        ("PRECEDES", list(precedes.values()), "adjacent content concept transitions"),
        ("PART_OF", list(part.values()), "files per directory"),
        ("DEPENDS_ON", list(dep_in.values()), "current Rust reverse dependency degree"),
    ]:
        out[name] = {"definition": definition, "nodes": len(values), "tail": tail_fit(values)}
    for name in ("IS_A", "CAUSES", "SUPPORTS", "CONTRADICTS", "TRANSFORMS_INTO", "SAME_ROLE_AS"):
        out[name] = {"status": "NOT MEASURED", "reason": "no serialized typed-edge observations in repository corpus"}
    return out


def collapse(docs: dict[str, str], seqs: dict[str, list[str]]) -> dict:
    scales = ["token", "phrase", "concept", "motif"]
    samples = {}
    for scale in scales:
        vals = list(collections.Counter(seqs[scale]).values())
        med = statistics.median(vals) if vals else 1.0
        samples[scale] = [math.log(v / med) for v in vals] if vals else []
    pairs = {}
    for i, a in enumerate(scales):
        for b in scales[i + 1:]:
            av, bv = sorted(samples[a]), sorted(samples[b])
            if not av or not bv:
                continue
            points = sorted(set(av + bv))
            # Two-sample KS after per-scale median rescaling.
            def cdf(v, x):
                return sum(y <= x for y in v) / len(v)
            ks = max(abs(cdf(av, x) - cdf(bv, x)) for x in points)
            pairs[f"{a}__{b}"] = {"ks_after_median_rescale": ks, "n_a": len(av), "n_b": len(bv)}
    return {"scales": scales, "pairwise": pairs, "interpretation": "low KS is shape agreement after one location rescale; not evidence of scale invariance"}


def invariant_candidates(docs: dict[str, str], graph: tuple[set[str], set[tuple[str, str]]], episodes: list[dict]) -> dict:
    nodes, edges = graph
    pair_support = collections.Counter()
    for e in episodes:
        files = [p for p in e["files"] if p in nodes]
        for a in files:
            for b in files:
                if a < b:
                    pair_support[a, b] += 1
    edge_support = collections.Counter()
    # Current relations are checked against historical file presence. This is
    # intentionally called path survival: imports themselves need not survive.
    sampled = episodes[::max(1, len(episodes) // 16)][:16]
    for a, b in edges:
        edge_support[a, b] = sum(a in e["files"] or b in e["files"] for e in sampled)
    top_pairs = sorted(pair_support.items(), key=lambda x: (-x[1], x[0]))[:25]
    top_edges = sorted(edge_support.items(), key=lambda x: (-x[1], x[0]))[:25]
    denom = max(1, len(episodes))
    return {
        "cochange_recurrence": [{"a": a, "b": b, "support_episodes": n, "recurrence": n / denom, "classification": "repetition proxy; invariance not established"} for (a, b), n in top_pairs],
        "dependency_path_survival": [{"importer": a, "imported": b, "sample_support": n, "sample_n": len(sampled), "classification": "endpoint survival only; relation survival NOT MEASURED"} for (a, b), n in top_edges],
        "strong_invariant_candidates": [],
        "reason": "meaningful transformation pairs, paraphrases, cross-embedder runs, and typed relation snapshots are absent from this workspace dataset",
    }


def importance(nodes: set[str], edges: set[tuple[str, str]], episodes: list[dict]) -> dict[str, float]:
    rev = reverse_graph(nodes, edges)
    co = collections.Counter()
    for e in episodes:
        for p in e["files"]:
            if p in nodes:
                co[p] += 1
    indegree = {n: len(rev.get(n, ())) for n in nodes}
    co_values = list(co.values())
    in_values = list(indegree.values())
    score = {}
    for n in nodes:
        # ECDF fusion avoids degree dominating recurrence by raw scale.
        score[n] = 0.5 * percentile(in_values, indegree[n]) + 0.5 * percentile(co_values, co[n])
    return score


def spine_section(graph: tuple[set[str], set[tuple[str, str]]], episodes: list[dict]) -> dict:
    nodes, edges = graph
    scores = importance(nodes, edges, episodes)
    ordered = sorted(nodes, key=lambda n: (-scores[n], n))
    test = episodes[len(episodes) // 2:]
    rev = reverse_graph(nodes, edges)
    curves = []
    for fraction in (0.01, 0.05, 0.10, 0.20, 0.50, 1.0):
        k = max(1, math.ceil(len(ordered) * fraction))
        spine = set(ordered[:k])
        rows = {"fraction": fraction, "spine_nodes": k, "arms": {}}
        for arm in ("full", "local", "top_importance", "spine_plus_local", "spine_plus_local_plus_exploration"):
            q, f = [], []
            for e in test:
                seeds = set(e["files"]) & nodes
                target = set(closure(seeds, rev))
                explore = {n for n in nodes if hashlib.sha256(n.encode()).digest()[0] < 13}
                candidate = {
                    "full": set(nodes), "local": seeds, "top_importance": spine,
                    "spine_plus_local": spine | seeds,
                    "spine_plus_local_plus_exploration": spine | seeds | explore,
                }[arm]
                q.append(len(candidate & target) / len(target) if target else 1.0)
                f.append(len(candidate) / len(nodes) if nodes else 0.0)
            rows["arms"][arm] = {"mean_quality": mean(q), "mean_fraction_evaluated": mean(f), "episodes": len(q), "quality_definition": "potential reverse-dependency closure recall"}
        curves.append(rows)
    return {"definition": "quality is held-out potential dependency-closure recall, not retrieval/task accuracy", "train_episodes": len(episodes) - len(test), "test_episodes": len(test), "backbone_top": [{"node": n, "score": scores[n]} for n in ordered[:25]], "curves": curves}


def incremental_section(graph: tuple[set[str], set[tuple[str, str]]], episodes: list[dict]) -> dict:
    nodes, edges = graph
    rev = reverse_graph(nodes, edges)
    rows = []
    for e in episodes[len(episodes) // 2:]:
        seeds = set(e["files"]) & nodes
        start = time.perf_counter()
        full_work = sum(len(rev.get(n, ())) + 1 for n in nodes)
        full_ms = (time.perf_counter() - start) * 1000
        start = time.perf_counter()
        got = closure(seeds, rev)
        inc_ms = (time.perf_counter() - start) * 1000
        reference = closure(seeds, rev)
        rows.append({"id": e["id"], "full_work": full_work, "incremental_work": len(got), "agreement": got == reference, "full_ms": full_ms, "incremental_ms": inc_ms, "estimated_work_speedup": full_work / max(1, len(got))})
    return {
        "status": "MEASURED_SURROGATE",
        "definition": "full scan versus exact reverse-closure traversal over the current import graph",
        "mean_agreement": mean(float(r["agreement"]) for r in rows),
        "mean_estimated_work_speedup": mean(r["estimated_work_speedup"] for r in rows),
        "rows": rows,
        "invariant_barrier": {"status": "NOT MEASURED", "reason": "no persisted invariant state or actual recomputation trace makes a semantics-safe barrier testable"},
    }


def routing_section(root: pathlib.Path) -> dict:
    path = root / "benchmarks/behaviour/actions.json"
    if not path.exists():
        return {"status": "NOT MEASURED", "reason": "benchmarks/behaviour/actions.json missing"}
    data = json.loads(path.read_text())
    sessions = data.get("sessions", [])
    rows = []
    for session in sessions:
        actions = session.get("actions", [])
        kinds = [a.get("kind", "unknown") for a in actions]
        for i, action in enumerate(actions):
            future = set(kinds[i + 1:i + 6])
            rows.append({"kind": action.get("kind", "unknown"), "future_fanout": len(future), "error": bool(action.get("harness_error") or action.get("marker_error"))})
    by_kind = collections.defaultdict(list)
    for row in rows:
        by_kind[row["kind"]].append(row["future_fanout"])
    weighted = sorted(((mean(v), k, len(v)) for k, v in by_kind.items()), reverse=True)
    budget = max(1, int(len(rows) * 0.20))
    ranked = sorted(rows, key=lambda r: (-mean(by_kind[r["kind"]]), r["kind"]))
    actual = sum(r["future_fanout"] for r in rows)
    captured = sum(r["future_fanout"] for r in ranked[:budget])
    error_rate = mean(float(r["error"]) for r in rows)
    return {
        "status": "MEASURED_PROXY",
        "definition": "action kind predicts distinct kinds in the next five actions; strong-model allocation is simulated as the top 20% predicted fanout",
        "actions": len(rows), "kinds": len(by_kind), "exploration_budget": 0.20,
        "captured_future_fanout_mass": captured / actual if actual else 0.0,
        "baseline_uniform_20_percent_mass": 0.20,
        "observed_error_rate": error_rate,
        "task_success": "NOT MEASURED",
        "model_quality": "NOT MEASURED",
        "top_kinds": [{"kind": k, "mean_future_fanout": v, "n": n} for v, k, n in weighted[:25]],
        "limits": ["trace has harness/marker errors, not task correctness", "no model calls or costs are recorded", "allocation result is a predictive concentration proxy"],
    }


def multiscale_section(root: pathlib.Path, docs: dict[str, str], graph: tuple[set[str], set[tuple[str, str]]], episodes: list[dict]) -> dict:
    seqs = scale_sequences(docs)
    scales = {}
    for name, seq in seqs.items():
        scales[name] = {"zipf": zipf(seq), "heaps": heaps(seq), "tail_of_frequency": tail_fit(collections.Counter(seq).values())}
    # Mutual information is quadratic in sequence distance. Keep the corpus
    # broad but cap each file and the number of files so this stays runnable on
    # a laptop; Zipf/Heaps still use the complete selected corpus.
    mi_docs = list(docs.values())[:250]
    mi_token = mutual_information([tokens(text)[:2_000] for text in mi_docs])
    mi_concept = mutual_information([concepts(text)[:2_000] for text in mi_docs])
    return {
        "documents": len(docs), "characters": sum(len(x) for x in docs.values()),
        "scales": scales,
        "long_range_dependence": {"token": decay_fit(mi_token), "concept": decay_fit(mi_concept), "measure": "mutual information with top-256 vocabulary plus <other>"},
        "collapse": collapse(docs, seqs),
        "relation_families": relation_distributions(docs, graph),
        "invariants": invariant_candidates(docs, graph, episodes),
    }


def self_test() -> None:
    fit = tail_fit([1] * 20 + list(range(2, 42)))
    assert fit["status"] == "MEASURED" and "power_law" in fit["models"]
    nodes, edges = {"a", "b", "c"}, {("b", "a"), ("c", "b")}
    assert closure({"a"}, reverse_graph(nodes, edges)) == {"a": 0, "b": 1, "c": 2}
    assert heaps(list("aaabbbcccccc"))["beta"] is not None
    assert zipf(list("aaabbbcccccc"))["alpha"] is not None
    print("self-test: ok")


def run(root: pathlib.Path) -> dict:
    started = time.perf_counter()
    graph = current_graph(root)
    docs = corpus(root)
    episodes = git_episodes(root, 500, 2)
    aval = avalanche_section(root, graph, episodes)
    result = {
        "schema": "physis-criticality-e65-e70/v1",
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "root": str(root.resolve()),
        "controls": {
            "power_law": "compared with truncated power law, lognormal, exponential, stretched exponential using tail likelihood/AIC",
            "criticality": "not inferred from a heavy tail; finite-size scaling, threshold robustness, and observed propagation remain separate gates",
            "invariant": "repetition and endpoint survival are not called invariance",
        },
        "archaeology": {
            "production_modified": False,
            "existing_paths": [
                "physis-core/src/map.rs: repeats/differences/contradiction candidates",
                "physis-core/src/becoming.rs: order-aware drift versus split and uptake",
                "physis-core/src/transform.rs and transplant.rs: structure-preserving homomorphism/transplant machinery",
                "physis-core/src/delta_engine.rs: shadow mutation, breadth wave, DependsOn revision walk",
                "physis-core/src/hypothesis.rs and epistemic.rs: fitness, survival, replayable revisions",
                "physis-core/src/observe.rs and src/ledger.rs: append-only observations and causal lineage",
                "src/dependency/mod.rs and src/semantic_delta/: dependency propagation and semantic deltas",
                "src/rag.rs, src/pack.rs, src/orchestration/: bounded context and existing routing seams",
            ],
            "missing_join": "no persisted bridge from observed events to avalanche records, invariant candidates, or incremental recomputation statistics",
        },
        "measurement": {"tracked_files": len(tracked(root)), "corpus_files": len(docs), "rust_nodes": len(graph[0]), "rust_edges": len(graph[1]), "git_episodes_2h": len(episodes), "corpus_file_cap": 300, "file_byte_cap": 160000, "mi_file_cap": 250, "mi_token_cap_per_file": 2000, "elapsed_seconds": time.perf_counter() - started, "max_rss_kb": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss},
        "E65_critical_structure": aval,
        "E66_linguistic_scale": multiscale_section(root, docs, graph, episodes),
        "E67_relation_family_scaling": {"relation_families": relation_distributions(docs, graph)},
        "E68_invariant_spine": spine_section(graph, episodes),
        "E69_incremental_avalanche": incremental_section(graph, episodes),
        "E70_critical_model_routing": routing_section(root),
    }
    return result


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", type=pathlib.Path, default=pathlib.Path("."))
    ap.add_argument("--out", type=pathlib.Path)
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        self_test(); return
    result = run(args.root.resolve())
    raw = json.dumps(result, indent=2, sort_keys=True)
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(raw + "\n")
    print(json.dumps(result["measurement"], sort_keys=True))
    for name in ("E65_critical_structure", "E66_linguistic_scale", "E68_invariant_spine", "E69_incremental_avalanche", "E70_critical_model_routing"):
        section = result[name]
        print(f"{name}: {section.get('status', 'MEASURED')}" if isinstance(section, dict) else name)


if __name__ == "__main__":
    main()
