# E82 — can the vector field tell a synonym from an antonym?

Yes, there is a dictionary: WordNet is already on disk (`models/wordnet`, 53 MB
of standard `data.*` / `index.*` files) and the crate lists `wordnet-db` as a
dev dependency. `benchmarks/conatus/wordnet.py` parses it directly — no nltk, no
network — and exposes synonyms, antonyms and hypernyms:

```
117791 synsets · 147982 words · 7983 antonym pointers
increase → synonyms  addition, gain, growth, increment, step-up
           antonyms  decrease, decrement, diminish, fall, lessen, reduction
           hypernyms alter, change, modify, change magnitude, physical process
```

The question that decides whether the symbolic layer earns its cost: does it
carry anything the geometry does not? Synonymy versus antonymy is the sharpest
case — *increase/growth* and *increase/decrease* are about the same subject
matter, and a distributional embedder is trained on exactly that co-occurrence.

## Method

200 pairs of each kind, drawn from WordNet and never from the embedder:
synonym (same synset), antonym (`!` pointer), hypernym (`@` pointer), and random
pairs as the floor. Each pair scored by cosine distance, twice: as **bare
words**, and with both words placed in a neutral **sentence frame** (bge is a
sentence encoder; a bare token is out of distribution for it). Reported as AUC —
can a reader holding only the distance say which pairs are synonyms?

## Result

| | synonym | hypernym | antonym | random | AUC syn vs anto |
|---|---:|---:|---:|---:|---:|
| bare words | 0.2978 | 0.3777 | 0.3041 | 0.4682 | **0.488** |
| sentence frame | 0.0840 | 0.1127 | 0.1153 | 0.1525 | **0.691** |

**Bare words: the geometry cannot tell them apart.** Synonym 0.2978 and antonym
0.3041 are the same distance to within a tenth of their spread, and AUC 0.488 is
chance. The field is not blind — synonym vs random is 0.910 and antonym vs
random 0.964 — it knows *related from unrelated* and not *which relation*.

**Context buys part of the distinction and does not close it, and it is not
stable.** In one frame the AUC rises to 0.691. In a second frame with the same
pairs it is **0.586**. So some of what looked like "context helps" is one
sentence helping.

**A mis-pairing control says the remainder is relational, not lexical.**
Re-pairing each first word with another pair's second word collapses the AUC to
**0.536**. What survives is about the actual relation between the two words, not
about the vocabulary they are drawn from.

**Verdict: the dictionary carries a distinction the field mostly lacks.** Bare
geometry is at chance on synonym-versus-antonym; context lifts it to 0.59–0.69
depending on the sentence, which is not enough to act on. For contradiction
detection — the thing physis's `Contradicts` relation is for — an antonym
pointer is 7983 facts the embedder does not have.

## A negative on the obvious next step

The tempting use of a thesaurus is to expand the cue dictionary from E81:
take each anchored cue verb and add its WordNet synonyms, anchored the same way.
Measured on `docs/`:

| dictionary | cues | relations | shuffled null | ratio |
|---|---:|---:|---:|---:|
| anchored (E81 v2) | 42 | 195 | 47 | **4.15** |
| + WordNet synonyms | 62 | 200 | 56 | 3.57 |

**Twenty more cues bought five more relations and cost 0.58 of the ratio.** The
synonyms WordNet offers for *requires*, *produces*, *measures* are rare in
technical prose, and each one is another chance to fire on a shuffle. Recall was
not the binding constraint; the anchoring was, and that was already fixed.

## Limits

One embedder (bge-base-en-v1.5). Frames are hand-written, and the two used here
differ enough to move the AUC by 0.105 — a systematic frame study would need
many. WordNet pairs are sampled from whichever words came first in a shuffled
index, not balanced by part of speech or frequency; antonym pointers concentrate
in adjectives, so the antonym sample is not a random sample of the language.
Nothing here tests the pipeline's *argument* extraction, which E81 left untested
and which is still untested.
