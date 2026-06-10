# Physis as the Attention Manager — Notebook + Agora

How the physis engine manages **Praxis Notebook** (attention that *produces*) and
**Agora** (attention that is *spent*) as two halves of one system. Mirrored in
`physis-pro` and `praxis_webapp` (`docs/PHYSIS_ATTENTION_ARCHITECTURE.md`).

## Unifying model

One embedding space, one 30-cell semiotic grid, one coherence index (+1/0/−1),
two opposite attention modes:

```
        physis engine  (classify · cohere · holarchy · dream · OEE)
        ▲                                                  ▲
        │ produce (+coherence)               spend (−coherence risk)
   ┌────┴─────┐                                  ┌─────────┴────┐
   │ NOTEBOOK │  ◄──── holarchy bridge ────►     │    AGORA     │
   │ journal  │   "what you consumed fed         │ visual feed  │
   │ (output) │    what you made"                │ (consumption)│
   └──────────┘                                  └──────────────┘
```

Praxis points attention at output; Agora is where attention is spent. Same brain
scores both, so physis can manage the whole attention budget, not just tag items.

## Notebook — managed PDCA log

Already wired: `PhysisAutoTagService` (classify → domain + tags),
`NotebookRhymeService` (cosine rhymes + dream echoes + pdca). The manager step:

1. **Entry → coherence node with a verdict** *(shipped)*. On create,
   `notebookRoutes` fires `registerCoherence('notebook:<id>', content, verdict, userId)`
   where `verdict ∈ {+1,0,−1}` from a mood/goal heuristic. Entries now seed the
   user's physis tenant, so `/pdca/plan` and `/dream` reason over the real
   life-log (the "Praxis backfill" in `PHYSIS_PRO_USECASES`).
2. **PDCA over the journal**: stagnation detection (`actor.rs`) surfaces "you
   circle X every Monday with 0 progress" as a brief.
3. **Weekly synthesis**: batch `/dream/generate` over recent entries → "these
   three notes are the same unsolved problem" → one proposed action.

## Agora — ToS-safe curation brain

Legal reality (`agora/LEGAL_FINDINGS.md`): aggregation + content-adjacent ads
violates source ToS → pivot is **subscription + standalone ad interstitial**.
Physis is what justifies the subscription:

1. **Affinity rank by your own coherence history** — candidate post → embed →
   cosine vs your `+1` nodes (what actually fed your output). Rank by that, not
   engagement.
2. **Quality gate, not engagement gate** — `/classify` quality score, then
   down-rank cells you marked inert. Anti-doomscroll by construction.
3. **Consume→produce bridge** — when a post correlates (holarchy edge) with an
   open notebook goal, tag it "feeds: <goal>". Agora becomes goal-fuel. The
   cross-product no pure journal or pure feed app can do.
4. **Govern the budget** — track time-in-feed; when session coherence trends
   −1 (pure spend), trigger the ad-interstitial / a "you're scrolling" nudge.

## Attention-OEE — one management number

The P0 OEE module generalises: "machine" = the user's day.

```
Availability = focused_time / available_time          (un-fragmented)
Performance  = signal / total_consumed                (quality-ranked hits)
Quality      = consumption_that_converted_to_output(+1) / total
Attention-OEE = A × P × Q
```

Notebook feeds Quality (the +1 conversions); Agora feeds the consumption counts.
Same `oee.rs` math via the stateless `/api/v1/oee/compute` endpoint.

## Wiring — all via the public proxy

Everything routes through `praxisweb.xyz/api/physis/v1/*` (allowlisted proxy, see
`PHYSIS_PUBLIC_ARCHITECTURE.md`). Reused engine endpoints: `classify`,
`coherence/register`, `coherence/snapshot`, `pdca/plan`, `dream/generate`,
`holarchy/snapshot|edge`, `reconstruct`, and the stateless `oee/compute`.

## Stateful vs stateless — runs online AND locally

The engine runs **multiple replicas** in the cloud, so in-memory state
(`OeeBoard`, `MaintenanceMonitor`) shards per replica and resets on redeploy.
Rule: **persistence lives in the caller (Praxis/Postgres); physis computes
statelessly.**

| Need | Endpoint | State |
|------|----------|-------|
| OEE report | `POST /api/v1/oee/compute` (body = full shift) | none — replica-safe |
| Plant rollup | `POST /api/v1/oee/rollup` (body = reports[]) | none |
| Train baseline | `POST /api/v1/maintenance/fold` (body = baseline?+reading → baseline) | none — caller persists baseline |
| Anomaly score | `POST /api/v1/maintenance/score` (body = baseline+reading) | none |
| RUL trend | `POST /api/v1/maintenance/rul` (body = samples+limit) | none |
| Local convenience | `POST /oee/event`, `/maintenance/observe` (in-memory board) | per-replica — single-instance/local only |

Same applies to attention data: Postgres holds the day's focus/consume/convert
counts; physis computes Attention-OEE on demand. Identical behaviour online and
on a laptop.

## Monetization

- **Notebook → Pro**: physis weekly synthesis + PDCA briefs (`requirePro`).
- **Agora → subscription**: coherence-ranked goal-aware feed + Attention-OEE
  dashboard. Physis *is* the reason to pay vs a free ad-feed, and sidesteps the
  ToS ad problem entirely.
- Metered through the `axiomGate('physis_api')` seam on the `/api/physis` mount.
