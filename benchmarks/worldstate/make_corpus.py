#!/usr/bin/env python3
"""Author the frozen WorldState corpus (E55/E56).

The corpus is one small evolving world — a machine shop — written as an ordered
text. **The temporal coordinate is `pos`, the sentence's ordinal position in the
text, not a clock.** Order is intrinsic to any corpus, needs no extraction, and
makes the shuffle null exact.

Every sentence is hand-authored here rather than sampled from a template, so
the structure is not a side effect of a generator. What the script adds is
determinism: it emits the same JSONL and the same sha256 every run, which is
what "frozen before evaluation" means.

Fields per record:
    pos         ordinal position in the text (the temporal coordinate)
    id          stable id, "s{pos:03d}"
    text        the sentence
    entities    entity ids mentioned
    relation    [subject, predicate, object] gold typed relation, or null
    transition  persist | change | emerge | disappear | recur | null
    split       train | heldout   (split BY ENTITY, see below)
    tag         null | adv_lex | adv_struct | agree_dup

Tags:
    adv_lex     high lexical overlap with its pair, structurally unrelated or
                inverted — the case ordinary similarity retrieval gets wrong
    adv_struct  structurally related to its pair, lexically disjoint — the case
                ordinary similarity retrieval misses
    agree_dup   a near-duplicate that AGREES with an earlier sentence. Mandatory
                negatives: without them a contradiction metric scores similarity
                and nothing else (learned by discarding a run in C2).

Usage:  python3 benchmarks/worldstate/make_corpus.py
Writes: corpus.jsonl, corpus.sha256
"""
import hashlib
import json
import pathlib

HERE = pathlib.Path(__file__).resolve().parent

# Entities. The held-out split is BY ENTITY so an alignment cannot memorise
# item identity: every sentence whose entity set touches HELDOUT_ENTITIES is
# held out, and no gold pair crosses the split.
HELDOUT_ENTITIES = {"conveyor", "sensor-7", "oil-tank"}

# (text, entities, relation, transition, tag, pair)
S = [
    # --- the world is introduced -------------------------------------------
    ("Lathe-3 stands at the north wall of the shop.", ["lathe-3"], None, "emerge", None, None),
    ("Press-2 stands beside it.", ["press-2"], None, "emerge", None, None),
    ("Lathe-3 requires coolant to cut steel without scoring.", ["lathe-3", "coolant"], ["lathe-3", "Requires", "coolant"], "emerge", None, None),
    ("The coolant pump feeds lathe-3 from the oil tank.", ["coolant-pump", "lathe-3", "oil-tank"], ["coolant-pump", "Supports", "lathe-3"], "emerge", None, None),
    ("Filter-A sits in the return line of the coolant pump.", ["filter-a", "coolant-pump"], ["filter-a", "Supports", "coolant-pump"], "emerge", None, None),
    ("Operator Rossi runs the north wall on shift B.", ["operator-rossi", "shift-b"], ["operator-rossi", "Supports", "shift-b"], "emerge", None, None),
    ("The spindle of lathe-3 turns at two thousand revolutions per minute.", ["spindle", "lathe-3"], ["spindle", "Supports", "lathe-3"], "emerge", None, None),
    ("A conveyor carries finished parts to the inspection bench.", ["conveyor"], None, "emerge", None, None),
    ("Sensor-7 reads the temperature of the spindle bearing.", ["sensor-7", "spindle"], ["sensor-7", "Supports", "spindle"], "emerge", None, None),
    ("The oil tank holds forty litres when full.", ["oil-tank"], None, "emerge", None, None),

    # --- steady state, then the first drift --------------------------------
    ("Lathe-3 still requires coolant to cut steel without scoring.", ["lathe-3", "coolant"], ["lathe-3", "Requires", "coolant"], "persist", "agree_dup", "A1"),
    ("The coolant pump continues to feed lathe-3.", ["coolant-pump", "lathe-3"], ["coolant-pump", "Supports", "lathe-3"], "persist", "agree_dup", "A2"),
    ("Swarf accumulates in the return line.", ["swarf"], None, "emerge", None, None),
    ("Filter-A begins to clog.", ["filter-a"], None, "change", None, None),
    ("Filter-A clogs the coolant pump.", ["filter-a", "coolant-pump"], ["filter-a", "Causes", "coolant-pump"], "change", "adv_lex", "L1"),
    ("The coolant pump clogs filter-A.", ["coolant-pump", "filter-a"], ["coolant-pump", "Causes", "filter-a"], "change", "adv_lex", "L1"),
    ("Chips obstruct circulation through the mesh.", ["swarf", "filter-a"], ["swarf", "Causes", "filter-a"], "persist", "adv_struct", "T1"),
    ("Flow to the north wall falls by a third.", ["coolant"], None, "change", "adv_struct", "T1"),
    ("Coolant pressure at lathe-3 drops below the set point.", ["coolant", "lathe-3"], ["coolant", "Influences", "lathe-3"], "change", None, None),
    ("Sensor-7 reports a rising bearing temperature.", ["sensor-7", "spindle"], ["sensor-7", "Influences", "spindle"], "change", None, None),

    # --- the operator responds ---------------------------------------------
    ("Rossi slows the spindle to fifteen hundred revolutions per minute.", ["operator-rossi", "spindle"], ["operator-rossi", "Causes", "spindle"], "change", None, None),
    ("The bearing temperature stops rising.", ["spindle"], None, "change", None, None),
    ("Rossi logs the pressure drop against lathe-3.", ["operator-rossi", "lathe-3"], ["operator-rossi", "Supports", "lathe-3"], "emerge", None, None),
    ("Maintenance is scheduled for the end of shift B.", ["shift-b"], None, "emerge", None, None),
    ("Press-2 keeps running at full rate.", ["press-2"], None, "persist", None, None),
    ("The conveyor keeps carrying parts to inspection.", ["conveyor"], None, "persist", "agree_dup", "A3"),
    ("Rossi slows the spindle further before the changeover.", ["operator-rossi", "spindle"], ["operator-rossi", "Causes", "spindle"], "recur", None, None),
    ("The spindle now turns at eight hundred revolutions per minute.", ["spindle"], None, "change", None, None),
    ("Scoring appears on two finished shafts.", ["lathe-3"], None, "emerge", None, None),
    ("Inspection rejects both shafts at the bench.", ["conveyor"], None, "change", None, None),

    # --- the intervention ---------------------------------------------------
    ("Filter-A is removed from the return line.", ["filter-a"], None, "disappear", None, None),
    ("A clean filter is fitted in its place.", ["filter-b", "filter-a"], ["filter-b", "Supports", "coolant-pump"], "emerge", None, None),
    ("Coolant pressure returns to the set point.", ["coolant"], None, "change", None, None),
    ("Lathe-3 cuts steel without scoring again.", ["lathe-3"], None, "recur", None, None),
    ("Rossi restores the spindle to two thousand revolutions per minute.", ["operator-rossi", "spindle"], ["operator-rossi", "Causes", "spindle"], "recur", None, None),
    ("Sensor-7 reads a stable bearing temperature.", ["sensor-7", "spindle"], ["sensor-7", "Supports", "spindle"], "recur", None, None),
    ("The oil tank is topped up to forty litres.", ["oil-tank"], None, "recur", None, None),
    ("Swarf no longer accumulates in the return line.", ["swarf"], None, "disappear", None, None),
    ("The shop returns to its ordinary rate.", [], None, "recur", None, None),
    ("Rossi closes the maintenance record for shift B.", ["operator-rossi", "shift-b"], ["operator-rossi", "Supports", "shift-b"], "change", None, None),

    # --- second cycle: the same shape on a different machine ---------------
    ("Press-2 requires hydraulic oil to hold ram pressure.", ["press-2", "oil"], ["press-2", "Requires", "oil"], "emerge", None, None),
    ("The oil tank feeds press-2 through the second line.", ["oil-tank", "press-2"], ["oil-tank", "Supports", "press-2"], "emerge", None, None),
    ("A strainer sits in the second line.", ["strainer"], None, "emerge", None, None),
    ("Fines accumulate in the second line.", ["fines"], None, "emerge", None, None),
    ("The strainer begins to clog.", ["strainer"], None, "change", None, None),
    ("The strainer clogs the oil tank line.", ["strainer", "oil-tank"], ["strainer", "Causes", "oil-tank"], "change", "adv_lex", "L2"),
    ("The oil tank line clogs the strainer.", ["oil-tank", "strainer"], ["oil-tank", "Causes", "strainer"], "change", "adv_lex", "L2"),
    ("Particles impede passage through the gauze.", ["fines", "strainer"], ["fines", "Causes", "strainer"], "persist", "adv_struct", "T2"),
    ("Ram pressure at press-2 falls below tolerance.", ["oil", "press-2"], ["oil", "Influences", "press-2"], "change", "adv_struct", "T2"),
    ("Press-2 misses depth on three pressings.", ["press-2"], None, "change", None, None),

    # --- contradiction and its agreeing twin --------------------------------
    ("Rossi reports that press-2 holds tolerance on every pressing.", ["operator-rossi", "press-2"], ["operator-rossi", "Contradicts", "press-2"], "change", None, None),
    ("Rossi reports that press-2 misses depth on three pressings.", ["operator-rossi", "press-2"], ["operator-rossi", "Supports", "press-2"], "persist", "agree_dup", "A4"),
    ("The strainer is removed and cleaned.", ["strainer"], None, "disappear", None, None),
    ("A cleaned strainer is refitted in the second line.", ["strainer"], None, "recur", None, None),
    ("Ram pressure at press-2 returns to tolerance.", ["oil", "press-2"], ["oil", "Influences", "press-2"], "recur", None, None),
    ("Press-2 holds depth on the next twenty pressings.", ["press-2"], None, "recur", None, None),
    ("Fines no longer accumulate in the second line.", ["fines"], None, "disappear", None, None),
    ("Rossi records both interventions in the same log.", ["operator-rossi"], None, "persist", None, None),
    ("The same failure shape appears twice in one week.", [], None, "recur", None, None),
    ("Maintenance proposes a shorter filter interval for both lines.", [], None, "emerge", None, None),

    # --- third cycle: held-out entities carry it ----------------------------
    ("The conveyor belt slips under a heavier load.", ["conveyor"], None, "change", None, None),
    ("Sensor-7 is re-tasked to read the conveyor drive current.", ["sensor-7", "conveyor"], ["sensor-7", "Supports", "conveyor"], "change", None, None),
    ("Drive current on the conveyor rises above its usual band.", ["conveyor"], None, "change", None, None),
    ("The conveyor drive current rises above its usual band.", ["conveyor"], None, "persist", "agree_dup", "A5"),
    ("The belt tensioner is adjusted by half a turn.", ["conveyor"], None, "change", None, None),
    ("Slippage on the conveyor stops.", ["conveyor"], None, "disappear", None, None),
    ("Drive current returns to its usual band.", ["conveyor"], None, "recur", None, None),
    ("The oil tank is drained for its quarterly inspection.", ["oil-tank"], None, "change", None, None),
    ("Press-2 stands idle while the oil tank is empty.", ["press-2", "oil-tank"], ["oil-tank", "Influences", "press-2"], "change", None, None),
    ("The oil tank is refilled and press-2 restarts.", ["oil-tank", "press-2"], ["oil-tank", "Supports", "press-2"], "recur", None, None),
    ("Sensor-7 returns to reading the spindle bearing.", ["sensor-7", "spindle"], ["sensor-7", "Supports", "spindle"], "recur", None, None),
    ("The shop ends the week at its ordinary rate.", [], None, "recur", None, None),
]


def build():
    out = []
    for pos, (text, ents, rel, trans, tag, pair) in enumerate(S):
        split = "heldout" if set(ents) & HELDOUT_ENTITIES else "train"
        out.append(
            {
                "pos": pos,
                "id": f"s{pos:03d}",
                "text": text,
                "entities": ents,
                "relation": rel,
                "transition": trans,
                "split": split,
                "tag": tag,
                "pair": pair,
            }
        )
    return out


def main():
    recs = build()
    body = "".join(json.dumps(r, sort_keys=True) + "\n" for r in recs)
    (HERE / "corpus.jsonl").write_text(body)
    digest = hashlib.sha256(body.encode()).hexdigest()
    (HERE / "corpus.sha256").write_text(digest + "  corpus.jsonl\n")

    n_train = sum(1 for r in recs if r["split"] == "train")
    tags = {}
    for r in recs:
        tags[r["tag"]] = tags.get(r["tag"], 0) + 1
    print(f"{len(recs)} sentences  train={n_train} heldout={len(recs) - n_train}")
    print("tags:", {k: v for k, v in sorted(tags.items(), key=lambda kv: str(kv[0]))})
    print("relations:", sum(1 for r in recs if r["relation"]))
    print("sha256:", digest)


if __name__ == "__main__":
    main()
