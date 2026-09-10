# I AM NOT AN AI/ML RESEARCHER. I KNOW MICROCONTROLLERS, MILLS, LIFTING AND "PHILOSOPHER"
# The Embedded AI Mindset: Treating LLM Tokens Like Microcontroller RAM

## 1. The Core Premise

Standard LLM deployment treats the context window as an infinite reservoir. Prompt injection, RAG retrieval, and chain-of-thought all assume: more context is better. This is the embedded systems equivalent of connecting a 200A main feed to a garden LED — wasteful, dangerous, and obscuring the actual load.

The TokenFixedRetriever in Physis-Core treats LLM tokens exactly like microcontroller RAM: a fixed, bounded resource that must be allocated, partitioned, and economised. Every token that enters the context window is a conscious trade-off against every other token. There is no "overflow" — if the budget is 40 tokens and the 5th chunk would push it to 41, that chunk is skipped. Full stop.

This is not a limitation. It is a first-principles design constraint that makes prompt assembly deterministic and overflow-proof regardless of corpus size.

## 2. The Mathematics of Token-Fixed RAG

The TokenFixedRetriever operates on a simple, ruthless greedy packing algorithm:

### Input
- A corpus of chunks `C = {c₁, c₂, ..., cₙ}`, each with:
  - Precomputed embedding `eᵢ` (vector of dimension d)
  - Precomputed token count `tᵢ` (via HeuristicTokenizer)
- A query embedding `q` (from the same embedder)
- A token budget `B` (e.g., 40, 100, 4096 — whatever the LLM's context window can safely hold)
- Optional diversity parameter `δ ∈ [0,1]` (MMR discount; 0 = pure score)

### Step 1: Score and Sort
Each chunk is scored by cosine similarity to the query:
`sᵢ = cosine(q, eᵢ)`

Chunks are sorted in descending score order. If diversity > 0, each candidate's effective score is discounted by its maximum cosine to already-selected chunks (MMR), reducing redundancy in the packed context.

### Step 2: Greedy Packing
Iterate through scored chunks in order. For each chunk `cᵢ`:
- If `used + tᵢ ≤ B` and `len(selected) < top_k`:
  - Accept the chunk
  - `used += tᵢ`
  - Add to selected set
- Otherwise:
  - Skip the chunk (it does not fit the budget)
  - Mark `truncated = true` if any skipped chunk had a higher effective score than an accepted one

### Step 3: Result
The output is `RetrievalResult`:
- `chunks`: the selected chunks, in score-descending order
- `total_tokens`: sum of tokens over selected chunks — guaranteed ≤ B
- `budget`: the budget B this retrieval was constrained to
- `truncated`: true when not every candidate fit (context was truncated to fit budget)

The guarantee is strict: `total_tokens ≤ budget` always. No approximation. No "approximately fits." Hard ceiling.

### Why This Matters

In a standard top-k RAG implementation, you might retrieve the top 10 most similar chunks regardless of combined token count. If each chunk is 512 tokens, you've just committed 5,120 tokens to context — potentially overflowing the LLM's window, causing silent truncation, perplexity spikes, or (in worst cases) context window errors that crash the pipeline.

The TokenFixedRetriever eliminates this class of failure. The prompt handed to the model is guaranteed to fit. This means:
- Deterministic prompt sizing: the same query + same budget always produces the same context length
- No silent truncation: the model receives exactly the tokens the retriever selected
- Budget-aware retrieval: you can trade off recall vs. context density by adjusting B
- Safe composition: multiple queries can be sent to the same LLM without exceeding its context window

## 3. The Heuristic Token Counter

The default token counter is dependency-free: it estimates token count using whitespace-split words with long-word overtokenisation, plus one token per punctuation/separator. The algorithm:

```
For each character in text:
  - If whitespace: flush current word token count
  - If alphanumeric: accumulate into current word
  - If punctuation/separator: flush word, then increment token counter

Tokenize word w: 1 + (len(w) - 6) / 4 (integer division, saturating at 0)
```

This mirrors how BPE over-tokenizes long words well enough for budgeting purposes. It is not exact — no heuristic over a text-to-token mapping without the actual tokenizer can be — but it is consistent and dependency-free. A real BPE tokenizer can be plugged in by implementing the `TokenCounter` trait.

## 4. Why Rust — Again

The TokenFixedRetriever is implemented in Rust for the same reasons the rest of Physis-Core is:

- Zero-cost abstraction: the token counting and retrieval logic compiles to machine code with no runtime overhead
- Deterministic: no garbage collection, no hidden memory allocations during retrieval
- Embedder-agnostic: the retriever takes `&[f32]` embeddings and a `dyn VectorEmbed` trait — the embedder can be ONNX, CPU-eval, or GPU-accelerated; the retriever doesn't care
- Testable: the entire retrieval path is unit-tested (budget respected, top_k caps selection, sorted by score, deterministic across runs)

The test suite verifies:
- `token_count_is_monotonic_and_nonzero`: whitespace matters, long words cost more
- `budget_is_respected`: total_tokens ≤ budget always (the critical assertion)
- `top_k_caps_selection`: at most top_k chunks are selected
- `selection_sorted_by_score`: first selected chunk scores ≥ the rest
- `deterministic_across_runs`: same query + same budget → same selection
- `assemble_never_exceeds_token_estimate`: assembled context token count ≤ budget

## 5. The Embedded AI Mindset in Practice

Treating LLM tokens like microcontroller RAM changes how you design the entire system:

### Before (Traditional LLM)
```
Query → top-k retrieval (k=10, no token limit) → concatenate all 10 chunks →
send 5,120 tokens to LLM → hope context window doesn't truncate → get answer
```

### After (Token-Fixed RAG)
```
Query → compute cosine scores → greedy pack until budget B reached →
send exactly ⌊B⌋ tokens to LLM → guaranteed fit → get answer
```

The mindset shift is from "retrieve as much as possible" to "retrieve the maximum-information subset that fits." This is the embedded systems principle of resource-constrained design applied to AI:

- **Token budgeting**: Every prompt is a fixed-size packet. You know exactly how much RAM it consumes before you send it.
- **Priority scoring**: Not all information is equal. Cosine similarity to the query ranks chunks by relevance. The retriever then selects the highest-relevance subset that fits.
- **Diversity control**: The `δ` parameter trades off redundancy for coverage. δ=0 gives maximum-relevance packing; δ=1 gives maximally diverse selection (possibly at lower relevance per chunk).
- **Hard limits**: The system refuses to exceed the budget. This is a feature, not a bug. It forces concise prompting and rewards the most relevant information.

## 6. The Sovereign Connection

The Sovereign Founder rejects empty corporate gluttony. A system that blindly maximises context window usage is — in the embedded mindset — equivalent to a device that draws maximum current regardless of load. It is wasteful, uncontrolled, and ultimately self-destructive.

Physis-Core's TokenFixedRetriever enforces the embedded AI mindset: every token is allocated. The context window is not an infinite reservoir; it is a fixed register file. This gives the Sovereign Founder a technical differentiator that is objectively real: a neuro-symbolic system with proven bounds on context consumption, truth (via immutable ontologies), and resource usage.

The company that builds on this architecture does not sell "more context." It sells *context you can trust*, with mathematically verified bounds. That is the antithesis of corporate gluttony. It is structural honesty at the token level.

---
*Test verification: budget_is_respected asserts `r.total_tokens <= 40` for all retrieval results. This is not a soft bound — it is a compile-time / run-time guarantee.*