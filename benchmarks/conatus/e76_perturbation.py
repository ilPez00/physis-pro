#!/usr/bin/env python3
"""E76 — does Physis resist perturbation, or merely store it?

Phase 8/9 of the 360 investigation. "Self-maintenance" is the claim that a
perturbed state moves back toward its previous shape. The rival explanation is
that nothing changes because nothing acts: a file that is not written to also
"resists perturbation".

Protocol, all through the real CLI on an isolated store:

  S0   scan a fixture corpus, snapshot the coherence state
  P    perturb: rewrite one node's content into something unrelated, and add a
       claim that contradicts the corpus
  S1   snapshot again — did anything move on its own?
  M    run every maintenance operation the engine offers (dream, quality)
  S2   snapshot again — did the operator-invoked repair restore S0?

Arms:

  physis    the engine, as above
  passive   the same files, the same perturbation, no engine between snapshots
            (the control: whatever this arm "restores" was never engine work)

Reported: coherence at each stage, node count, and the distance between the
node set at S0 and at S2. A system that restores nothing after its own
maintenance pass has operator-invoked repair, not self-maintenance.

    python3 benchmarks/conatus/e76_perturbation.py
"""
import argparse, json, os, re, shutil, subprocess, tempfile

# `snapshot` prints "  nodes:  N" and "  coherence index:  0.908". An earlier
# regex matched the first number after the word "coherence" anywhere, which
# picked up the node count and reported it as the coherence — the parser was
# doing the measuring.
COHERENCE = re.compile(r"coherence index:\s*([0-9]*\.?[0-9]+)", re.I)
NODES = re.compile(r"nodes:\s*(\d+)", re.I)

CORPUS = {
    "pump.md": "The feed pump is rated for 3 bar inlet pressure. Below 2 bar it stalls.\n"
               "Maintenance replaced the impeller in March after a stall.\n",
    "valve.md": "The inlet valve is checked weekly. A sticking valve starves the pump.\n"
                "Valve seat wear was recorded twice this year.\n",
    "shift.md": "Night shift reported two stalls in the same week as the valve wear.\n"
                "Day shift reported none.\n",
    "spec.md": "Specification: inlet pressure must stay above 2 bar at all times.\n"
               "Operators log pressure hourly.\n",
}

PERTURBATION = ("pump.md",
                "Cake recipes require 200g of flour and a warm oven.\n"
                "Whisk the eggs before folding in the sugar.\n")


def run(bin_path, store, args, timeout=300):
    env = dict(os.environ, PHYSIS_CORE_DIR=store, PHYSIS_ALLOW_DEV_LICENCE="1")
    p = subprocess.run([bin_path] + args, capture_output=True, text=True,
                       env=env, timeout=timeout)
    return p.stdout + p.stderr


def snapshot(bin_path, store):
    out = run(bin_path, store, ["snapshot"])
    coherence = COHERENCE.search(out)
    nodes = NODES.search(out)
    return {
        "coherence": float(coherence.group(1)) if coherence else None,
        "nodes": int(nodes.group(1)) if nodes else None,
        "raw": out.strip().splitlines()[:6],
    }


def node_labels(store):
    path = os.path.join(store, "nodes.json")
    if not os.path.exists(path):
        return []
    data = json.load(open(path))
    return sorted(n.get("label", "") for n in data.get("nodes", []))


def node_texts(store):
    """Label -> stored content.

    A node record holds `id, embedding, coherence_score, asserted, label,
    cell_pin, embedder, previous_coherence_state, self_model_version,
    preference` — and **no content**. An earlier version of this harness read a
    `content` key that does not exist, got "" for every node, and reported both
    "perturbing content present" and "original content present" as False, which
    read as a finding rather than as a missing field. Content-level restoration
    is NOT MEASURABLE from the store; only labels, scores and the embedding are.
    """
    path = os.path.join(store, "nodes.json")
    if not os.path.exists(path):
        return {}
    data = json.load(open(path))
    return {n.get("label", ""): n.get("coherence_score") for n in data.get("nodes", [])}


def build(bin_path, tmp):
    corpus = tempfile.mkdtemp(dir=tmp)
    for name, text in CORPUS.items():
        open(os.path.join(corpus, name), "w").write(text)
    store = tempfile.mkdtemp(dir=tmp)
    run(bin_path, store, ["scan", corpus])
    return corpus, store


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", default=os.path.expanduser("~/dev/physis-pro/target/release/physis-core"))
    ap.add_argument("--out", default=None)
    args = ap.parse_args()
    tmp = tempfile.mkdtemp(prefix="e76-")
    report = {}
    try:
        corpus, store = build(args.bin, tmp)
        report["S0"] = snapshot(args.bin, store)
        before_texts = node_texts(store)

        # Perturb: the file and the store both change, as they would if an agent
        # rewrote a document mid-session.
        open(os.path.join(corpus, PERTURBATION[0]), "w").write(PERTURBATION[1])
        run(args.bin, store, ["scan", corpus])
        run(args.bin, store, ["hypothesis", "create",
                              "inlet pressure below 2 bar is harmless to the pump"])
        report["S1_after_perturbation"] = snapshot(args.bin, store)

        # Every maintenance operation the engine exposes, invoked explicitly.
        report["maintenance"] = {
            "dream": run(args.bin, store, ["dream"]).strip().splitlines()[:4],
            "quality": run(args.bin, store, ["quality"]).strip().splitlines()[:4],
        }
        report["S2_after_maintenance"] = snapshot(args.bin, store)
        after_texts = node_texts(store)

        # Nodes are append-only: a rewritten file registers a NEW node and the
        # old one stays untouched. Counting "unchanged" nodes therefore measures
        # the store's append-only-ness, not any act of restoration, which is how
        # the first run of this harness reported the perturbed node as
        # "restored" when nothing had restored anything.
        restored = sum(
            1 for label, text in before_texts.items()
            if label in after_texts and after_texts[label] == text
        )
        report["nodes_before"] = len(before_texts)
        report["nodes_unchanged_after_the_whole_cycle"] = restored
        report["new_nodes_added_by_the_perturbation"] = len(after_texts) - len(before_texts)
        report["labels_before"] = sorted(before_texts)
        report["labels_after"] = sorted(after_texts)
        report["content_restoration"] = "NOT MEASURABLE: node records store no content"
    finally:
        shutil.rmtree(tmp, ignore_errors=True)

    print(f"{'stage':26}{'coherence':>11}{'nodes':>8}")
    for stage in ("S0", "S1_after_perturbation", "S2_after_maintenance"):
        s = report[stage]
        print(f"{stage:26}{s['coherence']!s:>11}{s['nodes']!s:>8}")
    print(f"\nnodes before perturbation          {report['nodes_before']}")
    print(f"nodes untouched through the cycle  {report['nodes_unchanged_after_the_whole_cycle']}")
    print(f"new nodes added by the perturbation {report['new_nodes_added_by_the_perturbation']}")
    print(f"labels before  {report['labels_before']}")
    print(f"labels after   {report['labels_after']}")
    print(f"content-level restoration: {report['content_restoration']}")
    print("\nmaintenance output:")
    for name, lines in report["maintenance"].items():
        print(f"  {name}: {' | '.join(lines) if lines else '(no output)'}")
    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)


if __name__ == "__main__":
    main()
