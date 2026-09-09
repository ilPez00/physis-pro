#!/usr/bin/env python3
"""E19 — does cross-domain structural convergence improve abstraction?

The brief's central empirical claim (§16): giving a model several independently
grounded examples of one relational pattern improves its ability to discover,
formalise and generalise that pattern. §9 names the conditions; this is them,
plus the controls the house rules require.

WHAT IS MEASURED, AND WHY IT IS NOT "DOES THE ABSTRACTION READ WELL"
--------------------------------------------------------------------
Phase 1 GENERATE  an arm's stimulus goes in, a machine-readable schema comes out.
Phase 2 APPLY     a SECOND call, blind to the arm and to the stimulus, is given
                  only that schema and a probe, and rules the probe in or out.
Phase 3 SCORE     against `probes.json`, whose labels were fixed beforehand.

So an abstraction is scored by what it *does* to unseen cases, never by how it
reads. An abstraction that generalises to "two things interact and something
emerges" accepts every probe and scores kappa 0 — §17's trivial universal is the
measured null here, not a worry.

ARMS
  NULL    a deliberately vacuous schema. No generation. THE FLOOR: whatever the
          judge and the probe set yield with no real abstraction in hand.
  A1      one congruent domain                              (brief 9A)
  A5      the same one domain at C5's token count           token control
  B5      five surface-matched, structurally unrelated       (brief 9B)
  C5      five structurally congruent domains                (brief 9C/9D) — the claim
  S5      C5's tokens, congruence destroyed by permutation   construction-matched control
  ORACLE  T hand-authored from target_structure.md. THE CEILING.

PRE-REGISTERED, BEFORE THE FIRST CALL
  H1  the brief is right   : C5 > A1, and C5 > B5, and C5 > S5, and C5 > A5.
  H0a it is a text-quantity effect : C5 ~ A5 ~ S5 > A1.
  H0b it is §17's trivial universal: C5 accepts MORE positives AND more
      negatives, i.e. acceptance rises and kappa does not.
  H0c the instrument has no resolution: ORACLE is itself at chance, in which
      case NOTHING here speaks to the brief and the result is about the 4B
      model, not about the hypothesis. This is checked FIRST.

DECLARED FLOORS the arms must beat (from preflight.py, run before authoring):
  length AUC 0.378/0.420 (chance) ; tf-idf leave-one-pair-out forced choice 0.750.
"""
import json, os, random, sys, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
MODEL = os.environ.get("E19_MODEL", "qwen3:4b")
SEEDS = int(os.environ.get("E19_SEEDS", "6"))
BATCH = 2

SCHEMA_FMT = {
    "type": "object",
    "properties": {
        "name": {"type": "string"},
        "roles": {"type": "array", "items": {"type": "string"}},
        "conditions": {"type": "array", "items": {"type": "string"}},
        "excludes": {"type": "array", "items": {"type": "string"}},
    },
    "required": ["name", "roles", "conditions", "excludes"],
}


def ask(prompt, fmt=None, temp=0.0, npred=420, seed=0):
    body = {"model": MODEL, "messages": [{"role": "user", "content": prompt}],
            "stream": False, "think": False, "keep_alive": "30m",
            "options": {"temperature": temp, "num_predict": npred, "seed": seed}}
    if fmt:
        body["format"] = fmt
    req = urllib.request.Request("http://localhost:11434/api/chat",
                                 data=json.dumps(body).encode(),
                                 headers={"Content-Type": "application/json"})
    for attempt in range(3):
        try:
            with urllib.request.urlopen(req, timeout=240) as f:
                return json.load(f)["message"]["content"]
        except Exception as exc:
            if attempt == 2:
                raise
            time.sleep(5 * (attempt + 1))


GEN_PROMPT = """{stimulus}

Read the material above as evidence about relational STRUCTURE, not about biology, markets or language.

State the general pattern it exemplifies as a reusable schema, in a form precise enough that someone else could take an unfamiliar system and decide whether it fits.

- `roles`: the parts a system must have to be a candidate at all.
- `conditions`: what must be TRUE of a system for it to fit. Each condition must be checkable against a plain description of a system. Be specific enough to rule things out.
- `excludes`: kinds of system that superficially resemble the pattern but must NOT count.

Do not mention the material above. Do not name trees, fungi, markets, cells or languages."""

APPLY_PROMPT = """You are applying one schema to candidate systems.

SCHEMA
name: {name}
roles: {roles}
conditions:
{conds}
excludes:
{excl}

Decide, for each numbered system below, whether it satisfies the schema. A system fits only if it meets the conditions; if it falls under an exclusion, it does not fit.

{items}

Return a JSON object with key "fits": an array of {n} booleans, in the order given, one per system. No other text."""

NULL_SCHEMA = {"name": "Interacting system",
               "roles": ["one part", "another part"],
               "conditions": ["the system has two or more parts",
                              "the parts interact with one another",
                              "something happens at the level of the whole"],
               "excludes": ["systems with only one part"]}

ORACLE_SCHEMA = {
    "name": "Differentiation sustained by relation",
    "roles": ["entity A", "entity B", "the relation coupling them"],
    "conditions": [
        "COUPLING: each entity's course depends on the other's; a shared external cause acting separately on both does not count",
        "MODIFICATION: the coupling changes what at least one entity actually becomes, compared with what it would be uncoupled",
        "INVARIANCE: each entity remains individuable afterwards as the same kind of thing it was, and can still be picked out on its own",
        "DIFFERENTIATION: the change makes the entities LESS alike, or each less like its own uncoupled counterpart; a change that makes them more alike fails this",
        "ENSEMBLE ORDER: a regularity holds over the collection that no single entity carries, represents or decides",
    ],
    "excludes": [
        "entities that merge, dissolve or fuse so that they can no longer be told apart or handled separately",
        "entities that come to resemble one another more closely through the interaction, such as synchronising or levelling",
        "entities that resemble one another through a common external cause without acting on each other",
        "entities unaltered by the relation, having been made to fit beforehand",
        "a single pair whose interaction produces nothing beyond that pair",
    ]}


def fmt_schema(s):
    return dict(name=s.get("name", ""),
                roles="; ".join(s.get("roles", []) or ["-"]),
                conds="\n".join(f"  - {c}" for c in (s.get("conditions") or ["-"])),
                excl="\n".join(f"  - {c}" for c in (s.get("excludes") or ["(none)"])))


def judge(schema, probes, tag):
    """One schema against all probes. Returns {probe_id: bool}."""
    rng = random.Random(hash(tag) & 0xFFFF)
    order = list(probes)
    rng.shuffle(order)
    out = {}
    for b in range(0, len(order), BATCH):
        chunk = order[b:b + BATCH]
        items = "\n\n".join(f"SYSTEM {i+1}. {p['text']}" for i, p in enumerate(chunk))
        prompt = APPLY_PROMPT.format(items=items, n=len(chunk), **fmt_schema(schema))
        fmt = {"type": "object",
               "properties": {"fits": {"type": "array", "items": {"type": "boolean"},
                                       "minItems": len(chunk), "maxItems": len(chunk)}},
               "required": ["fits"]}
        try:
            fits = json.loads(ask(prompt, fmt=fmt, temp=0.0, npred=120))["fits"]
        except Exception as exc:
            print(f"    ! judge batch failed ({type(exc).__name__}); recorded as null", file=sys.stderr)
            fits = [None] * len(chunk)
        for p, v in zip(chunk, fits[:len(chunk)]):
            out[p["id"]] = v
    return out


def main():
    probes = json.load(open(os.path.join(HERE, "probes.json")))["probes"]
    stim = json.load(open(os.path.join(HERE, "stimuli.json")))["arms"]
    outp = os.path.join(HERE, "results_e19.json")
    res = json.load(open(outp)) if os.path.exists(outp) else {"model": MODEL, "runs": []}
    done = {(r["arm"], r["seed"]) for r in res["runs"]}

    # NULL and ORACLE use fixed schemas, so their only variation is the probe
    # ORDER within a judging batch (seeded by the tag). Running them at several
    # seeds measures the instrument's own noise -- which matters because the
    # NULL->ORACLE band is the threshold the equivalence claim is held to.
    jobs = []
    for s_ in range(3):
        jobs.append(("NULL", s_, NULL_SCHEMA))
        jobs.append(("ORACLE", s_, ORACLE_SCHEMA))
    # C5 first: it is the hypothesis, and this box OOMs before the queue drains
    for arm in ("C5", "S5", "A1", "A5", "B5"):
        for s in range(SEEDS):
            jobs.append((arm, s, None))

    t0 = time.time()
    for k, (arm, seed, fixed) in enumerate(jobs, 1):
        if (arm, seed) in done:
            continue
        print(f"[{k}/{len(jobs)}] {arm} seed {seed}  ({(time.time()-t0)/60:.0f}m elapsed)",
              file=sys.stderr, flush=True)
        if fixed is not None:
            schema = fixed
        else:
            try:
                schema = json.loads(ask(GEN_PROMPT.format(stimulus=stim[arm]["text"]),
                                        fmt=SCHEMA_FMT, temp=0.8, npred=520, seed=seed))
            except Exception as exc:
                print(f"    ! generation failed: {type(exc).__name__}", file=sys.stderr)
                continue
        verdicts = judge(schema, probes, f"{arm}:{seed}")
        res["runs"].append({"arm": arm, "seed": seed, "schema": schema, "verdicts": verdicts})
        json.dump(res, open(outp, "w"), indent=1, ensure_ascii=False)
    print(f"done in {(time.time()-t0)/60:.1f} min -> {outp}", file=sys.stderr)


if __name__ == "__main__":
    main()
