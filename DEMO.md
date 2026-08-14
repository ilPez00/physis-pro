# Live Demo — Physis Industrial Backoffice Intelligence

One command brings the whole product up as a live dashboard at `/demo`. Everything
on screen is computed **in-process by the real engine** — no external services,
no mock data files. The model is the bundled BGE-base; the demo seed drives the
actual HTTP API.

## Launch

```bash
./scripts/demo.sh                 # → http://127.0.0.1:19876/demo
PHYSIS_HOST=0.0.0.0 ./scripts/demo.sh   # demo on a second screen / tablet
```

First run builds the web binary (the ONNX warm-up takes ~90s before it binds —
that's normal, it's loading the real model).

## The 60-second walkthrough

1. **Open `/demo`.** The page boots against a clean engine: health chip "ok",
   embedder `bge-base · semantic`, machines grid empty.
2. **Click `Seed demo data`.** This is not a fixture — it calls the live API:
   - registers 4 machines and plays one shift of production + six-big-loss
     stops (`POST /oee/machine`, `/oee/event`);
   - feeds 2 goals and asks `/dream/generate` for proposals;
   - backfills a 14-day healthy baseline on `cnc-2`, then a monitor loop that
     catches a bearing fault — an **anomaly with a 2.5-day RUL projection** and
     a coherence node for the dream loop (`/maintenance/observe`,
     `/maintenance/check`);
   - registers the day's documents/notes as coherence nodes and **links them
     into the holarchy graph** with edges.
3. **Watch the panels come alive:**
   - **Plant OEE** — ring gauges + the six-big-losses Pareto; `presse-7` is the
     problem child at ~59% (breakdown + scrap), `tornio-1`/`fresa-3` solid.
   - **Machines** — per-machine OEE, factors, good/scrap; `world-class` badge.
     The **Predictive watch** banner tells the auto-kaizen story: detected →
     cohered → dreamed, no hand-written rules.
   - **Shift Report** — the live engine's Italian `Rapporto di turno` (per
     machine, plus the dreamed countermeasures).
   - **Operational Timeline** — the day's events with an incident banner.
   - **Semiotic Grid** — 5 × 14 ontology activation heatmap.
   - **Holarchy** — the shared judgment graph (nodes + edges).
   - **Classify** — paste any log line/observation; it maps onto the semiotic
     grid with scores. "the shop speaks in its own grammar" is the closer.
4. **Refresh** any time — the OEE and report panels auto-refresh every 12s.

## Notes

- **Rate limiter:** the demo launch sets `PHYSIS_RATE_LIMIT=600/60` because the
  seed + initial load burst (~70 requests in one minute) blows past the
  **production default of `60/60`**. That default is a real safety gate for
  deployed instances — don't lower it in prod; only the demo harness raises it.
- **Data dir:** `/demo` writes to the engine's `~/.physis` volume like any
  server. To demo on a clean slate: `HOME=~/demo-physis ./scripts/demo.sh`.
- **Embedder:** if `/demo` boots without the model (degraded random projection),
  run `./scripts/download_models.sh` — classify/semiotic panels are only worth
  showing against a semantic embedder.
- **The demo is honest:** every number is live-computed from the API — there is
  no pre-baked chart. If a panel shows "429", the operator exceeded the rate
  budget in that window (raise the env value on the next launch).

## Learning & teaching — `/study`

The same engine has a **Study Log + argument trainer** page at
**`http://<host>:<port>/study`** (physis-schola).

- **Study Log** — every file a student works on is logged with date/time,
  action (generated / edited / opened / deleted), directory, file type and
  place. "Seed study session" runs a believable study week through the real
  endpoints: a chronos **daily rhythm** ("edit notes/topology.md every 1.0d")
  and a **delete → incident flag** appear immediately. Per-user (an
  `X-Physis-User` id), so one student's log never bleeds into another's.
- **Recorder** — capture a live session instead of seeding:
  `physis-pro study ~/study --place Library` polls the directory and posts
  every generated/edited/deleted file to the running engine; `--once` records
  what's already on disk as "opened".
- **Elenchus trainer** — paste an essay, tag each sentence as claim / warrant /
  rebuttal, evaluate: every claim comes back **supported / contested /
  unsupported**, with the press-here (unsupported) and recalibrate-here
  (contested) lists. A coherent claim is warranted and unrefuted.