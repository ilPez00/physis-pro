#!/usr/bin/env python3
"""E2 — the mapper hypothesis, tested.

EXPERIMENTAL. Reads `research/mapper/corpus.json` (written by
`physis-core/examples/mapper_corpus_export.rs`) and the Qwen3-8B tokenizer.
Writes only into research/mapper/. Touches no product code.

THE HYPOTHESIS
--------------
Language can be treated initially as a relational representational topology
rather than a fixed semantic ontology; sparse grounding and corrective
interaction can progressively constrain that topology into a more useful map of
externally observable phenomena.

THE DECISIVE CRITERION, FIXED BEFORE RUNNING
--------------------------------------------
Not internal coherence. Whether progressive grounding makes the map
increasingly predictive of something OUTSIDE the map. Here that is the
human-authored 5x14 grid cell of a HELD-OUT entry: the cell is authored against
`docs/GRID_AXES.md` by people, is not derivable from the text distribution by
construction, and no arm ever sees a test entry's label.

WHAT AN N-GRAM IS HERE (step 1 of the ladder, and it is not arbitrary)
---------------------------------------------------------------------
  alphabet : bytes (Qwen3 is a BYTE-LEVEL BPE, 256 roots)
  unit     : one BPE subword token from Qwen3-8B's 151,643-entry vocabulary
  context  : the pre-tokenizer regex splits on words/punctuation first, so a
             token never spans a word boundary except through the leading-space
             marker
  entry    : (token id, surface, merge rank, merge parents)
  lookup   : returns the token ids of a text, and through the merge DAG their
             derivation ancestors

The MERGE RANK is the Qwen-derived statistic: merges are stored in the order BPE
learned them, so an early rank means the pair was frequent in Qwen's training
corpus. The MERGE DAG is the Qwen-derived relation: every token above the 256
byte roots was constructed from two earlier tokens. A tokenizer-only baseline
has the vocabulary but neither of these; a corpus-local n-gram model has none of
the three. That is what makes arm C "Qwen-derived" rather than generic.
"""
import argparse
import json
import os
import sys
import numpy as np

HERE = os.path.dirname(os.path.abspath(__file__))
CORPUS = os.path.join(HERE, "corpus.json")


# ─────────────────────────────────────────────────────────────────────────
# The Qwen3-8B n-gram representation dictionary
# ─────────────────────────────────────────────────────────────────────────
class QwenDictionary:
    """Vocabulary + merge ranks + merge DAG. No weights, no inference."""

    def __init__(self, tok_path):
        raw = json.load(open(tok_path))
        m = raw["model"]
        assert m["type"] == "BPE", f"expected BPE, got {m['type']}"
        self.vocab = m["vocab"]                      # surface -> id
        self.inv = {i: s for s, i in self.vocab.items()}
        # merge rank: position in the learned-merge list. Earlier = more
        # frequent in Qwen's training corpus.
        self.parent = {}     # child id -> (left id, right id)
        self.rank = {}       # child id -> merge rank
        for r, pair in enumerate(m["merges"]):
            left, right = (pair if isinstance(pair, list) else pair.split(" ", 1))
            child = left + right
            cid = self.vocab.get(child)
            if cid is None:
                continue
            lid, rid = self.vocab.get(left), self.vocab.get(right)
            if lid is None or rid is None:
                continue
            self.parent[cid] = (lid, rid)
            self.rank[cid] = r
        self.n_merges = len(self.parent)
        self.max_rank = max(self.rank.values()) if self.rank else 1

    def ancestors(self, tid, max_depth=6):
        """Derivation ancestors of a token through the merge DAG."""
        out, frontier = set(), [tid]
        for _ in range(max_depth):
            nxt = []
            for t in frontier:
                p = self.parent.get(t)
                if not p:
                    continue
                for q in p:
                    if q not in out:
                        out.add(q)
                        nxt.append(q)
            if not nxt:
                break
            frontier = nxt
        return out

    def rank_weight(self, tid):
        """Qwen-derived informativeness surrogate, needing no corpus counts.

        A late merge rank means BPE only found the pair worth merging after
        seeing a lot of text -- a rarer, more specific string. A byte root
        (no merge) is maximally generic. This is an IDF-shaped weight read off
        Qwen rather than estimated from the 658-entry corpus, which is the
        point: the corpus is too small to estimate it.
        """
        r = self.rank.get(tid)
        if r is None:
            return 0.25            # byte root: present but uninformative
        return 0.25 + np.log1p(r) / np.log1p(self.max_rank)


# ─────────────────────────────────────────────────────────────────────────
# Representations — the ablation ladder
# ─────────────────────────────────────────────────────────────────────────
def l2(m):
    n = np.linalg.norm(m, axis=1, keepdims=True)
    return m / np.maximum(n, 1e-9)


def build_counts(token_lists, min_df=2, max_features=None):
    """entry x token count matrix, pruned to tokens seen in >= min_df entries.

    `max_features` caps the vocabulary at the most frequent tokens. It exists
    for tractability, not for tuning: PPMI is V x V, so an uncapped vocabulary
    over a 16k-document corpus would be tens of gigabytes. The cap is stated
    with the result whenever it binds.
    """
    df = {}
    for toks in token_lists:
        for t in set(toks):
            df[t] = df.get(t, 0) + 1
    keep = [t for t, c in df.items() if c >= min_df]
    if max_features and len(keep) > max_features:
        keep = sorted(keep, key=lambda t: -df[t])[:max_features]
    keep = sorted(keep)
    idx = {t: j for j, t in enumerate(keep)}
    M = np.zeros((len(token_lists), len(keep)), dtype=np.float32)
    for i, toks in enumerate(token_lists):
        for t in toks:
            j = idx.get(t)
            if j is not None:
                M[i, j] += 1.0
    return M, keep, idx


def ppmi(M, eps=1e-9):
    """Positive pointwise mutual information over token co-occurrence in entries.

    This is the L1 association measure, stated explicitly rather than left as
    "correlation". C = M^T M is token-token co-occurrence across entries.
    """
    C = M.T @ M
    np.fill_diagonal(C, 0.0)
    total = C.sum()
    if total <= 0:
        return C
    row = C.sum(axis=1, keepdims=True)
    col = C.sum(axis=0, keepdims=True)
    expected = (row @ col) / max(total, eps)
    with np.errstate(divide="ignore", invalid="ignore"):
        P = np.log((C * total + eps) / (row @ col + eps))
    P[~np.isfinite(P)] = 0.0
    return np.maximum(P, 0.0).astype(np.float32)


def svd_reduce(X, d=256, seed=0):
    """Truncated SVD, so every arm is compared at the same dimensionality."""
    d = min(d, min(X.shape) - 1)
    if d < 2:
        return X
    # randomized range finder -- deterministic given the seed
    rng = np.random.default_rng(seed)
    Q = rng.standard_normal((X.shape[1], d + 10)).astype(np.float32)
    Y = X @ Q
    Q1, _ = np.linalg.qr(Y)
    B = Q1.T @ X
    Ub, S, Vt = np.linalg.svd(B, full_matrices=False)
    return (Q1 @ Ub)[:, :d] * S[:d]


# ─────────────────────────────────────────────────────────────────────────
# Grounding + propagation
# ─────────────────────────────────────────────────────────────────────────
def predict(rep, anchors, anchor_labels, targets, k=5):
    """Propagation rule: cosine kNN over the anchors, similarity-weighted vote.

    Deliberately simple and IDENTICAL for every arm, so a difference between
    arms is attributable to the representation rather than to the classifier.
    """
    if len(anchors) == 0:
        return [None] * len(targets)
    A = rep[anchors]
    T = rep[targets]
    S = T @ A.T
    kk = min(k, len(anchors))
    out = []
    for i in range(len(targets)):
        top = np.argpartition(-S[i], kk - 1)[:kk]
        votes = {}
        for j in top:
            w = max(float(S[i, j]), 0.0)
            lab = anchor_labels[j]
            votes[lab] = votes.get(lab, 0.0) + w
        out.append(max(votes, key=votes.get) if votes else None)
    return out


def score(pred, truth):
    cell = np.mean([p == t for p, t in zip(pred, truth)])
    dom = np.mean([(p.split("/")[0] if p else "") == t.split("/")[0]
                   for p, t in zip(pred, truth)])
    mod = np.mean([(p.split("/")[1] if p else "") == t.split("/")[1]
                   for p, t in zip(pred, truth)])
    return cell, dom, mod


def grounding_curve(rep, labels, train_pool, test_idx, ks, reps=12, knn=5, seed=0):
    """Accuracy on held-out entries as the number of human anchors grows."""
    rng = np.random.default_rng(seed)
    truth = [labels[i] for i in test_idx]
    rows = []
    for k in ks:
        if k > len(train_pool):
            continue
        accs = []
        for r in range(reps):
            anchors = rng.choice(train_pool, size=k, replace=False)
            al = [labels[i] for i in anchors]
            pred = predict(rep, anchors, al, test_idx, k=knn)
            accs.append(score(pred, truth))
        accs = np.array(accs)
        rows.append((k, accs[:, 0].mean(), accs[:, 0].std(),
                     accs[:, 1].mean(), accs[:, 2].mean()))
    return rows


# ─────────────────────────────────────────────────────────────────────────
# Label propagation over the map graph — the mechanism the hypothesis
# actually proposes. kNN-to-anchors ignores the map's topology entirely;
# this pins anchors and lets the labels diffuse through the UNLABELLED
# structure, which is the "pinning data points" claim stated literally.
# Zhou et al. (2004) normalized-Laplacian propagation, solved directly.
# ─────────────────────────────────────────────────────────────────────────
def build_graph(rep, knn=10, sigma=None):
    """Symmetric kNN affinity graph over entries, cosine similarity."""
    S = rep @ rep.T
    np.fill_diagonal(S, -np.inf)
    n = S.shape[0]
    W = np.zeros((n, n), dtype=np.float32)
    kk = min(knn, n - 1)
    for i in range(n):
        top = np.argpartition(-S[i], kk - 1)[:kk]
        for j in top:
            v = max(float(S[i, j]), 0.0)
            W[i, j] = v
            W[j, i] = max(W[j, i], v)
    return W


def propagate(W, anchors, anchor_labels, classes, alpha=0.9, iters=60):
    """Diffuse sparse anchor labels through the graph. Returns scores per class."""
    n = W.shape[0]
    d = W.sum(axis=1)
    d[d <= 0] = 1.0
    Dm = 1.0 / np.sqrt(d)
    S = (W * Dm[:, None]) * Dm[None, :]
    cidx = {c: j for j, c in enumerate(classes)}
    Y = np.zeros((n, len(classes)), dtype=np.float32)
    for i, lab in zip(anchors, anchor_labels):
        Y[i, cidx[lab]] = 1.0
    F = Y.copy()
    for _ in range(iters):
        F = alpha * (S @ F) + (1 - alpha) * Y
    return F


def propagate_predict(W, anchors, anchor_labels, targets, classes, alpha=0.9):
    F = propagate(W, anchors, anchor_labels, classes, alpha=alpha)
    out = []
    for t in targets:
        row = F[t]
        out.append(classes[int(np.argmax(row))] if row.max() > 0 else None)
    return out


def propagation_curve(rep, labels, train_pool, test_idx, ks, classes,
                      reps=12, knn=10, alpha=0.9, seed=0):
    W = build_graph(rep, knn=knn)
    rng = np.random.default_rng(seed)
    truth = [labels[i] for i in test_idx]
    rows = []
    for k in ks:
        if k > len(train_pool):
            continue
        accs = []
        for _ in range(reps):
            anchors = rng.choice(train_pool, size=k, replace=False)
            al = [labels[i] for i in anchors]
            pred = propagate_predict(W, anchors, al, test_idx, classes, alpha=alpha)
            accs.append(score(pred, truth))
        accs = np.array(accs)
        rows.append((k, accs[:, 0].mean(), accs[:, 0].std(),
                     accs[:, 1].mean(), accs[:, 2].mean()))
    return rows
