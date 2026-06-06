# Physis-Based Philosophical Machines

A survey of the machines built on the physis core, the pattern they share, and a
brainstorm of new ones. A "philosophical machine" here = a program that **runs a
perception → reduction → reflection loop**, treats its primitives as *earned
conclusions* (not given axioms), and **remembers** what it understood so the next
encounter goes deeper.

---

## The shared pattern

All of these enact the same three-beat cycle (euclid names it after Husserlian
intentionality; physis names it Compression / Coherence / Dream):

```
PERCEIVE  →  REDUCE          →  COMPOSE        →  ELABORATE / DREAM   →  REMEMBER
(modality)   (to primitives)    (relational      (deepen on repeat;     (coherence
                                 graph)           collide w/ memory)      weights + recall)
```

Two commitments make them "philosophical" rather than just pipelines:
1. **Primitives are conclusions.** The machine starts with no vocabulary; forms
   emerge from the data and are *certified* (physis: a node becomes `+1.0` only
   when it produces a real, detected effect; euclid: a polygon is recognized only
   after it survives point-topology).
2. **Incompleteness is preserved.** euclid bans circles (geometrically exhausted,
   no vertices to subdivide); physis keeps Inert/Failure nodes around to dream on
   rather than deleting them. The machine prefers a form it can still elaborate.

---

## The existing machines

### physis_pro — the text/ontology machine
- **Perceive:** text, logs, images, video, audio (shared embedding space).
- **Reduce:** classify against the 5×6 semiotic grid; register coherence nodes.
- **Compose:** the holarchy graph + the DynamicVectorTrie.
- **Elaborate/Dream:** the Dream Function replays Inert/Failure nodes to propose
  corrective Effective Actions; PDCA/Rachmaninov directives.
- **Remember:** labeled nodes (now content-recallable via `physis-pro-wiki`),
  per-user tenants, soon Spanner for durability.
- It is the **core** the others embed.

### aura / aura_x — the always-on observer
- **Perceive:** ambient sensors — audio, vision, face recognition — a Rust daemon
  (Tauri v2 + headless), `sensor_daemon`, `entity_store`.
- **Reduce/Compose/Dream:** delegates to physis (git dep) — PDCA engine, ontology,
  DreamEngine, trie over a sled DB.
- **Remember:** entity store + Praxis gantt; surfaces a live network/feed UI.
- Essentially physis with the world as its input stream instead of a journal.

### euclid — the geometric/visual machine
- **Perceive:** a camera frame (`perceive.py` — edge detection, contours, saliency).
- **Reduce:** contours → closed polygons → classified primitives (`recognize.py`;
  **rejects circles and degenerate shapes** — primitives must be elaborable).
- **Compose:** primitives → a spatial relational graph (who touches whom, how).
- **Elaborate:** `elaborate.py` fractally deepens the form by generation number —
  the *same* form drawn richer each time it's re-encountered → SVG (plotter stub).
- **Intellect:** `intellect.py` rasterizes its own SVG output and re-perceives it
  (reflection on its own act), then stores. Husserlian comprehension → action →
  intellection, fully offline on a Debian terminal.
- Memory keys forms by hash + generation + learned abstractions.

> The throughline: **euclid is to vision what physis is to text** — same loop,
> different primitives. aura is physis pointed at the live world.

---

## Think of others — new machines, same core

Each is: pick a **modality**, define the **reduction to primitives**, the
**elaboration on repeat**, and let the physis core do coherence + dream + memory.

1. **chronos — the temporal machine.** Perceive a calendar/event stream; reduce
   to *recurrence primitives* (rhythms, cycles, anniversaries); compose a
   temporal graph; elaborate by predicting the next instance; dream surfaces a
   broken rhythm before it breaks. (Praxis already has the data — notebook
   waypoints + tracker cadence.)

2. **elenchus — the dialogue machine.** (Socratic cross-examination — *not*
   "agora", which is the social network.) Perceive a conversation; reduce to
   *claim / warrant / rebuttal* primitives (Toulmin); compose an argument graph;
   elaborate by finding the missing premise; coherence = whether a claim ever
   produced a real effect; conflicts trigger physis's ConstructiveRefutation. A
   debate partner that gets sharper each time you argue a topic.

3. **lex — the legal/contract machine.** Perceive documents; reduce to
   *obligation / right / condition* primitives; compose an obligations graph;
   elaborate by simulating breach paths; dream finds the unhandled contingency.
   (physis already has `ingest` for invoices/contracts — this is the reasoning
   layer on top.)

4. **harmony — the music machine.** Perceive audio; reduce to *interval / motif*
   primitives (not notes — intervals are the elaborable unit, like euclid's
   polygons-not-circles); compose a motif graph; elaborate by fractal variation;
   memory = a personal style that deepens. (`physis-pro-voice` already does
   audio→graph.)

5. **soma — the body machine.** Perceive wearable signals (HR, motion, sleep);
   reduce to *effort / recovery / drift* primitives; compose a physiology graph;
   coherence = did a protocol produce a detected effect (the `register_
   behavioural_vector` Success/Inert/Failure model is already exactly this).
   Dream proposes the next training tweak.

6. **polis — the social-graph machine.** Perceive a community's posts/matches;
   reduce to *affinity / reciprocity* primitives; compose a relational graph;
   elaborate by predicting which ties strengthen; coherence-weighted (Praxis's
   `social::SocialAlgorithm` already scores engagement × coherence).

7. **techne — the codebase machine.** Perceive a repo (scan already does this);
   reduce to *capability / dependency* primitives beyond file paths; compose a
   capability graph; elaborate by tracing "what breaks if this changes"; dream
   flags an architectural smell. (Extends the current scan-labelling toward
   meaning, per Phase 6 "process embedding".)

**Common substrate for all of them:** the physis HTTP API + per-user tenants +
the coherence/dream loop. A new machine is mostly a *reducer* (modality →
primitives → labeled nodes/edges) plus an *elaborator* (how a known form deepens).
Everything downstream — recall, coherence, dream, durability (Spanner) — is shared.

---

## Why this matters for the roadmap

The wiki content-recall work (Phase 7) turned physis from "a classifier" into "a
memory you can browse." That memory is the substrate every machine above needs.
The next high-leverage core investment is the **stable id bridge** (coherence id
↔ holarchy node ↔ Spanner row) — once nodes, edges, and durable rows share an
identity, any of these machines can read/write the same graph. That bridge also
unblocks 7C's edge export (PREMORTEM P7).
