# Plan — Monitor mode: the phone as a perception device

**Status:** engine-side slice in progress; Android client not started.
Written 2026-08-30.

---

## The idea

A day leaves traces the person living it does not write down. The accelerometer
knows there was a workout. The GPS knows it happened at the gym and not the
office. The clock knows it was the fourth Tuesday in a row, and that the fifth
did not happen. None of that reaches the graph today, so the engine reasons
about documents and machines while the life the documents are *for* stays
invisible to it.

Monitor mode is a collector on the phone that turns sensor traces into the same
normalized events the rest of the engine already consumes.

## Why this is small

The instinct to build "a tracking app" produces a second product with its own
storage, its own inference, and its own screens. That is not what this is. The
split it needs — **device perceives, engine means** — is the boundary Phase 8b
closed: `POST /api/v1/machines/{name}/reduce` takes primitives and registers
them into the graph. A phone is another perception client.

Every reducer it needs already exists and is tested:

| Sensor | Machine | Primitive |
|---|---|---|
| accelerometer, step counter | `kinesis` → `soma` | `Motion` from gestures; `Session` with effort / recovery / **drift** |
| GPS fixes | `topos` | `detect_places(waypoints, radius_km, min_visits)` — recurring *places*, not a trace |
| every timestamp | `chronos` | rhythms, and **slipping** rhythms: the "you stopped doing this" signal |
| battery, screen-on, ambient light | `empedocles` | `Reading` against a baseline → stable / anomaly |
| self-reported mood | `pathos` | affect episodes |

Plus `FileAddress` for where/when and the causal ledger for why a record exists.
So the engine work is a **mapping layer and some widgets**, not new concepts.

The desktop precedent is `physis-interest-daemon`, which already watches
clipboard and directories — and already has the right instinct in its own
header: *"persists only a preview + hash (never the full text)"*.

## Pro and Normal are the ladder that already exists

Normal mode is the deterministic reducers: `detect_rhythms`, `detect_places`,
`detect_motions`, the cell classifier, the offline corpus summaries. They are
model-free and they are the product.

Pro mode is the provider cascade on top — `ask`, `corpus/ask`, dream generation
— which already degrades to ranked results with an explicit note when no
provider is configured.

**The rule: Pro adds synthesis, never capture.** Same sensors, same events, same
panels; Pro only phrases them. A Pro tier that can *see* something Normal cannot
is two products, and a user can trust neither.

## Display: widgets, not a firehose

"Show everything" is the instinct, and it is the thing this product's own
constraint rejects — *what is shown must be derived from state by construction*.
A wall of readings is not derived from anything; it is a dump next to the state.

Monitor mode therefore ships **notebook widgets**, not a page:
`sensor.today`, `places.recent`, `rhythms.slipping`, `workouts.week`. Each
person composes the ones their life actually has. Nothing is hidden and nobody
is shown a wall.

## The microphone, specifically

This is the one component with real legal exposure, and it is a design
constraint rather than a reason to drop the feature.

Recording yourself is your business. A "lesson" or a "call" captures **other
people**, which leaves the GDPR household exemption and, in Italy, engages
art. 615-bis / 617 c.p. on illicit interception. The liability is in *storing
the audio*. The value is in the inference, and the inference does not need it.

**Classify without recording:**

- On-device feature extraction only — RMS energy, spectral flux, voice-activity
  detection, a rough speaker-count estimate — computed on a rolling buffer that
  is discarded in the same pass.
- Never persist PCM. Never write an audio file. Never transcribe third parties
  by default.
- The event that reaches the graph is *"conversation, 2+ voices, 50 minutes,
  at the university, 14:00–14:50"*. That is what the user wanted; the audio was
  never the point.
- Transcription stays an explicit, per-session, foreground action the user
  starts and can see running.

This is also what makes the store listing defensible instead of alarming.

## Distribution

**Decision (2026-08-30, owner):** ship through **Play Store + direct APK from
git**. F-Droid is not a target, because F-Droid requires every dependency to be
free software and that rules out anything Google-shaped.

One correction worth recording, because it changes what the decision costs:
F-Droid's constraint is **licensing**, not permissions. Play's constraint is
**review**, and Play reviews background location and microphone access far more
harshly than F-Droid ever would — this app's two most sensitive permissions are
exactly the ones Play scrutinizes. So the trade is a licensing constraint for a
review constraint that is tighter for this specific app.

The useful consequence: **none of this needs Play Services.** `LocationManager`,
`SensorManager` and `AudioRecord` are all AOSP. Building without Play Services
costs nothing, works on Play, works from git, and leaves F-Droid open if the
review process turns out to be the harder road. So:

- No Play Services, no Firebase — not for F-Droid's sake, but because they buy
  nothing here.
- Direct APK from git is the primary channel (the audience for a self-hosted
  intelligence engine is comfortable with it).
- Play is the secondary channel; expect a data-safety declaration and a
  background-location justification, and write them from the design above.
- Pro mode points at **the user's own physis-pro instance**, not a service we
  run. That is the honest answer to every store's "where does the data go".

## Android realities to design for, not around

- Foreground service with declared types: `FOREGROUND_SERVICE_LOCATION`,
  `FOREGROUND_SERVICE_MICROPHONE`, `FOREGROUND_SERVICE_HEALTH` (Android 14+).
- `ACCESS_BACKGROUND_LOCATION` and `ACTIVITY_RECOGNITION` are separate grants
  with their own flows.
- Doze and battery optimization will kill naive polling; batch and sync.
- Android 14+ shows a permanent microphone/location indicator. **Embrace it.**
  An app that says "that dot is on because you asked me to notice your day"
  reads completely differently from one that hides it.

## Build order

| # | Step | Where |
|---|---|---|
| M1 | `monitor` module: sensor batch → machine primitives, pure and tested | engine |
| M2 | `POST /api/v1/monitor/ingest` — one call per sync, routes to the reducers, records causes | engine |
| M3 | Notebook widgets over what M2 produced | engine |
| M4 | Android foreground service: sensors → batched events → M2 | new repo |
| M5 | Audio feature extraction with no retention | new repo |
| M6 | Play data-safety declaration + APK release from git | packaging |

M1–M3 are this repo and are what "proceed" means today. M4–M6 are a separate
Kotlin project that talks to this one over HTTP, which is the whole point of
having closed the boundary first.

## Risks

- **A collector that collects nothing useful.** Mitigation: M1–M3 first, so the
  phone has a proven endpoint before anyone writes Kotlin against it.
- **Battery.** A monitor that halves battery life gets uninstalled whatever it
  knows. Batch aggressively; prefer the step counter (hardware, already
  counting) over raw accelerometer polling.
- **The graph filling with noise.** Every GPS fix is not an event. `topos`
  reduces fixes to recurring *places* and that reduction has to happen before
  registration, not after.
- **Scope drift into a fitness app.** This is a perception feed for an
  intelligence engine. The moment it grows its own training plans it is a
  different product competing in a crowded category.
