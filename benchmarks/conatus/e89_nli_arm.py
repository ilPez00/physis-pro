#!/usr/bin/env python3
"""E89 — the NLI arm on the conatus contradiction fixtures.

E84 scoped the model arm to the implication class the rules measurably fail
(0/3), leaving its comparison unrun. E86 added that NLI's precision on the
adversarial set is worth one run too, since the rules' collapse there is the
baseline to beat — and E85 weakened the high end of the NLI spectrum with a
data point from this repository's own memory: 120B beats cosine cleanly while
7B collapses.

The model is the smallest honest NLI cross-encoder with fp32 ONNX weights:
cross-encoder/nli-deberta-v3-xsmall (22M params, MNLI, labels
contradiction/entailment/neutral). Requires the venv (.venv-nli) and
models/nli-deberta-v3-xsmall. Threshold 0.5 on P(contradiction).

    .venv-nli/bin/python benchmarks/conatus/e89_nli_arm.py --out e89-results.json
"""
import argparse, json, os, sys, time
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
MD = os.path.join(os.path.dirname(HERE), os.pardir, "phys", "x")
MODEL_DIR = os.path.abspath(os.path.join(HERE, "..", "..", "models", "nli-deberta-v3-xsmall"))
FIXTURES = [
    ("holdout (E84)", os.path.join(HERE, "fixtures", "contradiction-holdout.json")),
    ("adversarial (E85)", os.path.join(HERE, "fixtures", "adversarial-consistent.json")),
    ("validation (E86)", os.path.join(HERE, "fixtures", "adversarial-consistent-2.json")),
]


def main():
    import onnxruntime as ort
    from tokenizers import Tokenizer
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=None)
    ap.add_argument("--threshold", type=float, default=0.5)
    args = ap.parse_args()
    sess = ort.InferenceSession(os.path.join(MODEL_DIR, "onnx", "model.onnx"),
                                providers=["CPUExecutionProvider"])
    tok = Tokenizer.from_file(os.path.join(MODEL_DIR, "tokenizer.json"))

    def p_contra(a, b):
        enc = tok.encode(a, b)
        ids = np.array([enc.ids], dtype=np.int64)
        mask = np.ones_like(ids)
        out = np.array(sess.run(None, {"input_ids": ids,
                                       "attention_mask": mask})[0][0])
        probs = np.exp(out - out.max())
        return float(probs[0] / probs.sum())

    report = {"model": "cross-encoder/nli-deberta-v3-xsmall (22M, MNLI, fp32 ONNX)",
              "threshold": args.threshold}
    for name, path in FIXTURES:
        items = json.load(open(path))["pairs"]
        t0 = time.time()
        scores = [p_contra(i["a"], i["b"]) for i in items]
        dt = (time.time() - t0) / max(1, len(items))
        tp = sum(1 for i, s in zip(items, scores) if i["contradiction"] and s >= args.threshold)
        fp = sum(1 for i, s in zip(items, scores) if not i["contradiction"] and s >= args.threshold)
        fn = sum(1 for i, s in zip(items, scores) if i["contradiction"] and s < args.threshold)
        precision = tp / (tp + fp) if tp + fp else 0.0
        recall = tp / (tp + fn) if tp + fn else 0.0
        print(f"\n=== {name}: {len(items)} pairs · P(contradiction) >= {args.threshold} "
              f"→ precision {precision:.3f} recall {recall:.3f} "
              f"({tp} caught, {fp} false+, {fn} missed) · {dt*1000:.0f} ms/pair")
        for i, s in zip(items, scores):
            if i["contradiction"] and s < args.threshold:
                print(f"  MISSED  [{i['class']}] P={s:.2f} {i['a']} || {i['b']}")
            if not i["contradiction"] and s >= args.threshold:
                print(f"  FALSE+  [{i['class']}] P={s:.2f} {i['a']} || {i['b']}")
        impl = [(i, s) for i, s in zip(items, scores) if i["class"] == "implication"]
        if impl:
            print("  implication scores:", ", ".join(f"{s:.2f}" for _, s in impl))
        report[name] = {"precision": precision, "recall": recall, "tp": tp,
                        "fp": fp, "fn": fn, "ms_per_pair": round(dt * 1000, 1),
                        "implication_scores": [round(s, 3) for _, s in impl]}
    if args.out:
        json.dump(report, open(args.out, "w"), indent=1)
        print(f"\nwrote {args.out}")


if __name__ == "__main__":
    main()
