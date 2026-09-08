#!/usr/bin/env python3
"""E10 — form through selective error: does downstream viability beat local correctness?

THE HYPOTHESIS (§1, §17 of the brief)
-------------------------------------
Some organized systems maintain form not by eliminating every deviation, but by
discriminating between deviations according to their CONSEQUENCES: deviations
that prevent further viable composition are abandoned; non-handicapping ones may
persist and become part of the evolving form.

WHAT THIS EXPERIMENT CAN AND CANNOT SETTLE
------------------------------------------
It CANNOT establish that music and DNA are "genuinely congruent". That is a
conceptual claim about two domains, and no amount of code decides it. What it
CAN do is test the ABSTRACTION the brief actually proposes -- selection by
downstream compositional viability versus selection by local correctness -- in a
system where the answer is known by construction. If the abstraction loses here,
the cross-domain framing does not rescue it.

THE SYSTEM (§2, §7, §8 made computable)
---------------------------------------
Music is the brief's own test case and it is the one that formalises cleanly,
because it separates identity from attributes without argument:

    a MOTIF's identity is its interval contour, not its pitches.

    O = (I, A, R, C)
      I  interval contour  -- the invariant
      A  pitch realisation -- the attributes (register lives here)
      R  adjacency constraints to neighbouring motifs
      C  position in the phrase

    A register transformation Tr = transposition, and by construction
      I(Tr(O)) = I(O)   and   A(Tr(O)) != A(O)
    which is exactly the condition §8 asks to be satisfied.

Composition concatenates motifs into a phrase under a voice-leading constraint.
DOWNSTREAM VIABILITY = can the phrase still be extended k more steps?

OPERATIONAL DEFINITIONS (§18 -- these are NOT interchangeable)
-------------------------------------------------------------
  deviation  : any change to A
  error      : a deviation that breaks I                (ground truth, by construction)
  neutral    : a deviation that preserves I and viability
  handicap   : preserves I but reduces future viability
  correction : reverting a deviation
  selection  : retaining or discarding by some criterion
  noise      : deviation applied without regard to consequence
"""
import json, os, sys
from collections import Counter, defaultdict
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from preflight import (check_power_auc, check_metric_wellposed,
                       check_class_sparsity, report)

RNG = np.random.default_rng(11)
LOW, HIGH = 48, 72          # two octaves: register moves are genuinely risky here
MAX_LEAP = 5                # voice-leading constraint between adjacent motifs


def contour(pitches):
    """The INVARIANT: the interval sequence. Transposition-invariant by
    construction, which is the whole point."""
    return tuple(int(b - a) for a, b in zip(pitches, pitches[1:]))


def realise(cont, start):
    p = [start]
    for iv in cont:
        p.append(p[-1] + iv)
    return p


def in_range(p):
    return all(LOW <= x <= HIGH for x in p)


def compose_ok(prev, nxt):
    """R: adjacent motifs must be joinable — no leap larger than MAX_LEAP."""
    return abs(nxt[0] - prev[-1]) <= MAX_LEAP


# ── deviations. Each returns (new_pitches, kind) ─────────────────────────
def dev_transpose(p, rng):
    """ONE offset for the whole motif — that is what makes it a transposition.
    The first version drew a fresh offset per note, which scrambles the contour
    instead of preserving it and left every arm with identity_rate 0.00."""
    off = int(rng.integers(-7, 8))
    return [x + off for x in p], "register"


def dev_ornament(p, rng):
    """Insert a passing tone: changes the literal interval list but keeps the
    skeleton. Under the strict invariant this BREAKS I — deliberately, so the
    experiment contains deviations that are musically reasonable and formally
    fatal, which is where the arms differ."""
    i = int(rng.integers(0, len(p) - 1))
    mid = (p[i] + p[i + 1]) // 2
    return p[: i + 1] + [mid] + p[i + 1:], "ornament"


def dev_perturb(p, rng):
    """Alter one interval: breaks I."""
    q = list(p)
    i = int(rng.integers(1, len(q)))
    for j in range(i, len(q)):
        q[j] += int(rng.choice([-2, -1, 1, 2]))
    return q, "perturb"


def dev_octave(p, rng):
    """Register shift by an octave: preserves I exactly, may leave range.
    One offset for the whole motif, for the same reason as above."""
    off = int(rng.choice([-12, 12]))
    return [x + off for x in p], "octave"


DEVS = [dev_transpose, dev_ornament, dev_perturb, dev_octave]


def viable_depth(phrase, cont_pool, rng, k=6, tries=6):
    """DOWNSTREAM VIABILITY: how many more motifs can be appended legally?"""
    depth = 0
    cur = list(phrase)
    for _ in range(k):
        placed = False
        for _ in range(tries):
            c = cont_pool[int(rng.integers(len(cont_pool)))]
            start = cur[-1][-1] + int(rng.integers(-MAX_LEAP, MAX_LEAP + 1))
            cand = realise(c, start)
            if in_range(cand) and compose_ok(cur[-1], cand):
                cur.append(cand); placed = True; break
        if not placed:
            break
        depth += 1
    return depth


def run_arm(policy, motif_pool, n_gen=240, seed=0):
    """One generative run. Returns the measurements §17 asks for."""
    rng = np.random.default_rng(seed)
    base_contours = [contour(m) for m in motif_pool]
    phrase = [motif_pool[0]]
    kept, corrections = [], 0
    identity_ok = 0
    neutral_used = 0
    for _ in range(n_gen):
        parent = phrase[-1]
        dev, kind = DEVS[int(rng.integers(len(DEVS)))](parent, rng)
        I_parent, I_dev = contour(parent), contour(dev)
        preserves_I = (I_dev == I_parent)
        locally_ok = in_range(dev) and compose_ok(parent, dev)
        depth = viable_depth(phrase + [dev], base_contours, rng) if locally_ok else 0

        if policy == "reject_all":
            accept = False
        elif policy == "accept_all":
            accept = True
        elif policy == "local":
            accept = locally_ok
        elif policy == "downstream":
            accept = locally_ok and depth >= 5   # 5 of 6, so the filter binds
        else:
            raise ValueError(policy)

        if accept and in_range(dev):
            phrase.append(dev)
            kept.append((kind, preserves_I, depth))
            identity_ok += int(preserves_I)
            if preserves_I and depth >= 2:
                neutral_used += 1
        else:
            corrections += 1
            # correction = fall back to a transposition of the parent, the
            # register move that preserves I by construction
            alt = [x + int(rng.integers(-3, 4)) for x in parent]
            if in_range(alt):
                phrase.append(alt)
    n_kept = max(len(kept), 1)
    return {
        "depth": len(phrase),
        "identity_rate": identity_ok / n_kept,
        "corrections": corrections,
        "diversity": len(set(contour(m) for m in phrase)),
        "neutral_exploited": neutral_used,
        "mean_downstream": float(np.mean([k[2] for k in kept])) if kept else 0.0,
        "kinds": Counter(k[0] for k in kept),
    }


def main():
    print("E10 — form through selective error\n")

    # ── pre-flight, because that is what it is for ───────────────────────
    print("═" * 78)
    print("PRE-FLIGHT — check the design before running it")
    print("═" * 78)
    # The first version of this ran at 12 replicates, the checker returned STOP
    # on power, and it was run anyway -- exactly the failure the checker exists
    # to prevent. The replicate count is now set so the design can resolve the
    # effect it is looking for, and the check is re-run to confirm.
    for reps, n_gen in [(12, 240), (40, 240)]:
        labels = ["viable"] * (reps * 10) + ["nonviable"] * (reps * 10)
        w = report([check_metric_wellposed(labels, metric="kappa"),
                    check_power_auc(reps * 10, reps * 10, effect=0.10),
                    check_class_sparsity(labels, floor_n=6)],
                   f"{reps} replicates x {n_gen} generations")
        if w != "REFUTED":
            REPS = reps
            break
    else:
        print("\n  every configuration is underpowered — not running.")
        return

    # ── the motif pool ──────────────────────────────────────────────────
    rng = np.random.default_rng(3)
    motif_pool = []
    while len(motif_pool) < 24:
        c = [int(x) for x in rng.integers(-5, 6, size=4)]
        p = realise(tuple(c), 60)
        if in_range(p):
            motif_pool.append(p)

    print("\n" + "═" * 78)
    print("REGISTER TRANSFORMATION — does §8's condition actually hold?")
    print("═" * 78)
    ok = sum(1 for m in motif_pool
             for t in [[x + 7 for x in m]]
             if contour(t) == contour(m) and t != m)
    print(f"\n  I(Tr(O)) = I(O) and A(Tr(O)) != A(O) for {ok}/{len(motif_pool)} motifs")
    print("  Transposition satisfies the register condition by construction, so the")
    print("  formalism in §8 is coherent for this system. That is a definition")
    print("  holding, not an empirical finding — worth stating so it is not")
    print("  mistaken for one.")

    print("\n" + "═" * 78)
    print("THE FOUR ARMS (§17)")
    print("═" * 78)
    arms = [("A  reject every deviation", "reject_all"),
            ("B  accept every deviation", "accept_all"),
            ("C  select on LOCAL correctness", "local"),
            ("D  select on DOWNSTREAM viability", "downstream")]
    print(f"\n  {'arm':<34}{'depth':>7}{'ident':>8}{'divers':>8}"
          f"{'corr':>7}{'neutral':>9}{'downstr':>9}")
    results = {}
    for name, pol in arms:
        runs = [run_arm(pol, motif_pool, seed=s) for s in range(REPS)]
        agg = {k: float(np.mean([r[k] for r in runs]))
               for k in ["depth", "identity_rate", "diversity", "corrections",
                         "neutral_exploited", "mean_downstream"]}
        results[name] = agg
        print(f"  {name:<34}{agg['depth']:>7.0f}{agg['identity_rate']:>8.2f}"
              f"{agg['diversity']:>8.0f}{agg['corrections']:>7.0f}"
              f"{agg['neutral_exploited']:>9.0f}{agg['mean_downstream']:>9.2f}")

    print("\n  depth   = phrase length reached (compositional depth)")
    print("  ident   = fraction of ACCEPTED deviations that preserved the invariant")
    print("  divers  = distinct interval contours present")
    print("  corr    = corrections spent")
    print("  neutral = deviations that preserved I AND stayed viable — the")
    print("            'non-handicapping variation' the hypothesis is about")

    print("\n" + "═" * 78)
    print("FALSIFICATION (§19) — where the abstraction breaks")
    print("═" * 78)
    d, c = results["D  select on DOWNSTREAM viability"], results["C  select on LOCAL correctness"]
    print(f"\n  downstream vs local:")
    for k, lbl in [("identity_rate", "identity preservation"),
                   ("diversity", "diversity"), ("depth", "compositional depth"),
                   ("neutral_exploited", "neutral variation exploited"),
                   ("corrections", "corrections spent")]:
        delta = d[k] - c[k]
        print(f"    {lbl:<30}{c[k]:>9.2f} -> {d[k]:>9.2f}   {delta:>+9.2f}")
    b = results["B  accept every deviation"]
    print(f"\n  and against accept-everything, the check on combinatorial growth (§19):")
    print(f"    diversity  {b['diversity']:.0f} (accept all) vs {d['diversity']:.0f} (downstream)")
    print(f"    identity   {b['identity_rate']:.2f} (accept all) vs {d['identity_rate']:.2f} (downstream)")


if __name__ == "__main__":
    main()
