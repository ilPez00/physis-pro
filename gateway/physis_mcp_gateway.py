#!/usr/bin/env python3
"""Physis MCP Gateway — exposes the vector-only Physis engine as MCP tools.

Runs as an MCP server over stdio. Manages the Rust sidecar lifecycle.
All data is represented as vectors — no human-readable qualia.

Set PHYSIS_API_URL to override the default (http://127.0.0.1:19876).
Set PHYSIS_BIN to override the physis-web binary path.
"""

import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import httpx
from mcp.server import FastMCP

PHYSIS_API_URL = os.environ.get("PHYSIS_API_URL", "http://127.0.0.1:19876")
PHYSIS_BIN = os.environ.get("PHYSIS_BIN", "")
PHYSIS_PORT = os.environ.get("PHYSIS_PORT", "19876")

_sidecar: subprocess.Popen | None = None


def find_binary() -> Path | None:
    if PHYSIS_BIN:
        p = Path(PHYSIS_BIN)
        if p.exists():
            return p.resolve()
    script_dir = Path(__file__).resolve().parent.parent
    candidates = [
        script_dir / "target" / "release" / "physis-web",
        script_dir / "target" / "debug" / "physis-web",
        script_dir / "target" / "release" / "physis" / "physis-web",
    ]
    for c in candidates:
        if c.exists():
            return c
    return None


def start_sidecar() -> subprocess.Popen | None:
    global _sidecar
    binary = find_binary()
    if binary is None:
        return None
    env = os.environ.copy()
    env["PHYSIS_PORT"] = PHYSIS_PORT
    proc = subprocess.Popen(
        [str(binary)],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=env,
    )
    _sidecar = proc
    for _ in range(20):
        try:
            r = httpx.get(f"{PHYSIS_API_URL}/health", timeout=2)
            if r.status_code == 200:
                return proc
        except Exception:
            pass
        time.sleep(0.5)
    proc.terminate()
    return None


def stop_sidecar():
    global _sidecar
    if _sidecar is not None:
        try:
            _sidecar.terminate()
            _sidecar.wait(timeout=5)
        except Exception:
            _sidecar.kill()
        _sidecar = None


mcp = FastMCP("physis")

_client: httpx.AsyncClient | None = None
_sidecar_started = False


def get_client() -> httpx.AsyncClient:
    global _client
    if _client is None:
        _client = httpx.AsyncClient(base_url=PHYSIS_API_URL, timeout=30)
    return _client


async def ensure_running():
    global _sidecar_started
    if _sidecar_started:
        return
    try:
        r = await get_client().get("/health", timeout=2)
        if r.status_code == 200:
            _sidecar_started = True
            return
    except Exception:
        pass
    proc = start_sidecar()
    if proc is not None:
        _sidecar_started = True
    else:
        print("WARNING: Physis sidecar not running and binary not found.", file=sys.stderr)
        print(f"Start it manually: physis-sidecar", file=sys.stderr)


async def api_get(path: str, params: dict | None = None) -> dict:
    await ensure_running()
    r = await get_client().get(path, params=params)
    r.raise_for_status()
    return r.json()


async def api_post(path: str, body: dict | None = None) -> dict:
    await ensure_running()
    r = await get_client().post(path, json=body or {})
    r.raise_for_status()
    return r.json()


# ── Tools ──────────────────────────────────────────────────────────────


@mcp.tool()
async def physis_health() -> str:
    """Check if the Physis engine is running."""
    try:
        data = await api_get("/health")
        return f"Physis engine: {data}"
    except Exception as e:
        return f"Physis engine not reachable: {e}"


@mcp.tool()
async def physis_stats() -> str:
    """Get Physis engine statistics — trie nodes, coherence bands, PDCA."""
    data = await api_get("/api/v1/stats")
    mapper = data.get("mapper", {})
    core = data.get("core", {})
    lines = [
        "=== Physis Stats ===",
        f"Trie nodes: {mapper.get('nodes', '?')}",
        f"Tokens: {mapper.get('tokens', '?')}",
        f"Max depth: {mapper.get('max_depth', '?')}",
        f"Human domains: {mapper.get('human_domains', '?')}",
        f"Machine domains: {mapper.get('machine_domains', '?')}",
        f"Coherence index: {core.get('coherence_index', 0):.3f}",
        f"Total nodes: {core.get('total_nodes', '?')}",
        f"High coherence: {core.get('high_coherence', '?')} / Mid: {core.get('mid_coherence', '?')} / Low: {core.get('low_coherence', '?')}",
        f"Certified branches: {core.get('certified_branches_count', '?')}",
        f"Isolated branches: {core.get('isolated_branches_count', '?')}",
        f"Dream cycles: {core.get('dream_cycle_count', '?')}",
        f"Clusters: {core.get('cluster_count', '?')} / Outliers: {core.get('outlier_count', '?')}",
    ]
    return "\n".join(lines)


@mcp.tool()
async def physis_query(query: str, max_results: int = 10) -> str:
    """Search the Physis UberWiki trie.

    Args:
        query: Search terms to find in the trie
        max_results: Maximum number of results to return (default 10)
    """
    data = await api_get("/api/v1/query", params={"q": query, "max": max_results})
    results = data.get("results", [])
    if not results:
        return f"No results found for: {query}"
    lines = [f"Found {data.get('count', 0)} results for '{query}':"]
    for path in results:
        lines.append(f"  {' → '.join(path)}")
    return "\n".join(lines)


@mcp.tool()
async def physis_scan(dir: str) -> str:
    """Scan a directory and index it into the Physis trie.

    Args:
        dir: Absolute path to the directory to scan
    """
    data = await api_post("/api/v1/scan", {"dir": dir})
    count = data.get("count", 0)
    if count == 0:
        return f"No files indexed from: {dir}"
    return f"Scanned {dir}\nIndexed {count} goals (vector embeddings + trie paths)"


@mcp.tool()
async def physis_ingest_prompt(prompt: str) -> str:
    """Register a user prompt as a vector Goal in the Physis engine.

    The prompt is embedded server-side into a 384-d vector. No qualia labels stored.

    Args:
        prompt: The full text of the user's prompt / request / task
    """
    data = await api_post("/api/v1/goals", {"prompt": prompt})
    goal = data.get("goal", {})
    gid = goal.get("id", "?")[:8]
    progress = goal.get("progress", 0.0)
    return f"Goal registered (id: {gid}...) progress={progress:.2f}"


@mcp.tool()
async def physis_dream_generate(count: int = 5, force: bool = False) -> str:
    """Generate stochastic dreams from the current goal vector set.

    Dreams are pure vector-space operations: interpolation, extrapolation, mutation.
    Dreaming is suppressed while PDCA cycles are active. Set force=True to override.

    Args:
        count: Number of dreams to generate (default 5)
        force: Bypass PDCA gating (default False)
    """
    data = await api_post("/api/v1/dream/generate", {"count": count, "force": force})
    active = data.get("active", False)
    if not active:
        reason = data.get("reason", "Dreaming is suppressed")
        return f"Dreaming inactive: {reason}"
    dreams = data.get("dreams", [])
    if not dreams:
        return "No dreams generated. Scan or ingest prompts first."
    lines = [f"Generated {data.get('count', 0)} dreams:"]
    for d in dreams:
        g = d.get("grade")
        grade_str = f" grade={g:.2f}" if g is not None else ""
        src = d.get("source", [])
        emb = d.get("embedding", [])
        src_preview = ",".join(f"{x:.2f}" for x in src[:3])
        emb_preview = ",".join(f"{x:.2f}" for x in emb[:3])
        lines.append(f"  [{d['id'][:8]}...] src=[{src_preview}...] emb=[{emb_preview}...]{grade_str}")
    return "\n".join(lines)


@mcp.tool()
async def physis_dream_evaluate(dream_id: str, grade: float) -> str:
    """Evaluate a dream with a grade. Accepted if grade >= 0.6.

    Args:
        dream_id: The ID of the dream to evaluate
        grade: Grade between 0.0 and 1.0
    """
    data = await api_post("/api/v1/dream/evaluate", {"id": dream_id, "grade": grade})
    accepted = data.get("accepted", False)
    status = "ACCEPTED" if accepted else "REJECTED"
    return f"Dream {dream_id[:8]}... evaluated at grade {grade:.2f} — {status}"


@mcp.tool()
async def physis_coherence_snapshot() -> str:
    """Get the coherence snapshot — band counts, branches, clusters."""
    data = await api_get("/api/v1/coherence/snapshot")
    return (
        f"Coherence Index: {data.get('coherence_index', 0):.3f}\n"
        f"Total Nodes: {data.get('total_nodes', '?')}\n"
        f"  High: {data.get('high_coherence', '?')}\n"
        f"  Mid:  {data.get('mid_coherence', '?')}\n"
        f"  Low:  {data.get('low_coherence', '?')}\n"
        f"Clusters: {data.get('cluster_count', '?')}\n"
        f"Outliers: {data.get('outlier_count', '?')}\n"
        f"Certified Branches: {data.get('certified_branches_count', '?')}\n"
        f"Isolated Branches: {data.get('isolated_branches_count', '?')}\n"
        f"Dream Cycles: {data.get('dream_cycle_count', '?')}"
    )


@mcp.tool()
async def physis_coherence_register(input: str) -> str:
    """Register a coherence node from raw text.

    The text is embedded server-side into a vector. Coherence is computed
    geometrically via cosine similarity to existing nodes.

    Args:
        input: Text describing the concept/task to register
    """
    data = await api_post("/api/v1/coherence/register", {"input": input})
    return f"Registered coherence node — id: {data.get('node_id', '?')[:8]}..."


@mcp.tool()
async def physis_context_filter(input: str) -> str:
    """Filter raw input through geometric context filter.

    Embeds the input and validates it against the coherence space. No qualia.

    Args:
        input: Raw text to filter
    """
    data = await api_post("/api/v1/context/filter", {"input": input})
    emb = data.get("embedding", [])[:4]
    emb_preview = ",".join(f"{x:.4f}" for x in emb)
    return (
        f"Valid: {data.get('valid', False)}\n"
        f"Token estimate: {data.get('token_estimate', 0)}\n"
        f"Embedding preview: [{emb_preview}...]"
    )


@mcp.tool()
async def physis_compress_logs(logs: list[str]) -> str:
    """Compress raw logs into dense causal rules.

    Args:
        logs: List of log strings to compress
    """
    data = await api_post("/api/v1/compress/logs", {"logs": logs})
    compressed = data.get("compressed", "")
    return (
        f"Compressed {data.get('input_count', 0)} logs\n"
        f"Output: {data.get('output_chars', 0)} chars\n"
        f"Rules: {compressed}"
    )


@mcp.tool()
async def physis_pdca_plan() -> str:
    """Get the PDCA plan — goals sorted by progress (lowest first)."""
    data = await api_get("/api/v1/pdca/plan")
    planned = data.get("planned", [])
    if not planned:
        return "No goals planned."
    lines = [f"PDCA Plan — {data.get('count', 0)} goals:"]
    for g in planned:
        pct = g.get("progress", 0.0) * 100
        gid = g.get("id", "?")[:8]
        lines.append(f"  [{gid}...] progress={pct:.0f}%")
    return "\n".join(lines)


@mcp.tool()
async def physis_pdca_act(goal_id: str, state_vector: list[float]) -> str:
    """Execute a PDCA action — records a state transition vector.

    Args:
        goal_id: The ID of the goal to act on
        state_vector: Vector representing the new state after action
    """
    data = await api_post("/api/v1/pdca/act", {
        "goal_id": goal_id, "state_vector": state_vector,
    })
    eid = data.get("experience_id", "?")[:8]
    return f"Experience recorded: {eid}..."


@mcp.tool()
async def physis_pdca_stats() -> str:
    """Get PDCA statistics — actions, progress, stagnant count."""
    data = await api_get("/api/v1/pdca/stats")
    lines = [
        f"Total actions: {data.get('total_actions', 0)}",
        f"Total goals: {data.get('total_goals', 0)}",
        f"Average grade: {data.get('avg_grade', 0.0):.3f}",
        f"Mean progress: {data.get('mean_progress', 0.0):.3f}",
    ]
    stagnant = data.get("stagnant_count", 0)
    if stagnant:
        lines.append(f"Stagnant goals: {stagnant}")
    return "\n".join(lines)


@mcp.tool()
async def physis_reconstruct(input: str) -> str:
    """Reconstruct semantic context from a query — finds nearest neighbor vectors.

    Embeds the input, finds the top-5 most similar coherence nodes by cosine similarity,
    and returns their embeddings and similarity scores.

    Args:
        input: Text to use as the reconstruction query
    """
    data = await api_post("/api/v1/reconstruct", {"input": input})
    query = data.get("query_embedding", [])[:4]
    qp = ",".join(f"{x:.4f}" for x in query)
    lines = [
        f"Query embedding preview: [{qp}...]",
        f"Total candidates: {data.get('count', 0)}",
    ]
    for n in data.get("neighbors", []):
        eid = n.get("id", "?")[:8]
        sim = n.get("similarity", "?")
        cs = n.get("coherence_score", "?")
        lines.append(f"  [{eid}...] sim={sim} coherence={cs}")
    return "\n".join(lines)


# ── Entrypoint ─────────────────────────────────────────────────────────

def main():
    try:
        mcp.run()
    finally:
        stop_sidecar()


if __name__ == "__main__":
    main()
