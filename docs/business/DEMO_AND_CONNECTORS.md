# Product demo and connector roadmap

This document turns Physis from a general engine into a demonstrable product
without claiming that every connector already exists.

## The demo: “one day in a small workshop”

Use a 3D printer as a safe, visible stand-in for a production machine. Run one
print while Physis receives four kinds of evidence:

1. **Machine state** — Moonraker telemetry from Klipper (the API used by
   Fluidd and Mainsail): temperatures, print state, progress, pause/resume,
   errors, and filament or heater events.
2. **Human workflow** — a small folder of shift notes, sliced-file metadata,
   maintenance notes, and a CSV of print jobs. `PHYSIS_WATCH_DIR` can ingest
   changes as they happen.
3. **Communication** — a handful of `.eml` messages representing an order,
   delay, approval, and resolution. The email feature extracts sender, subject,
   body, and a heuristic thread ID.
4. **Operator verdicts** — the operator marks an alert as useful, inert, or
   wrong. That asserted outcome is separate from semantic similarity and is
   the feedback loop.

The final screen/report should answer one operational question:

> “Why did this job take longer than expected, what evidence supports the
> diagnosis, and what should we do before the next job?”

The demo is successful when it produces a short timeline, an OEE-style loss
breakdown, the related documents/messages, and one human-approved action. It
does not need to pretend to predict a bearing failure from a single printer.

### Suggested scripted scenario

```text
08:55  job starts; bed reaches temperature slowly
09:12  nozzle temperature oscillates; print pauses
09:16  operator writes “cleaned nozzle, resumed”
09:30  supplier email says replacement nozzle ships tomorrow
10:40  job completes, with 14 minutes of interruption
```

Physis should connect the telemetry event, the note, and the email; classify the
failure; register the operator’s verdict; and generate a countermeasure such as
“inspect/replace nozzle before the next job.”

## 3D-printer integration: Fluidd/Mainsail

Yes, this is a good test environment. Fluidd and Mainsail are user interfaces;
the machine-facing service is normally **Moonraker**, which exposes HTTP and
JSON-RPC/WebSocket APIs for Klipper.

What exists today:

- generic `MachineReading` and anomaly detection;
- an MQTT adapter behind the `industrial` feature;
- HTTP maintenance/OEE endpoints;
- a Unix `AuraFrame` sensory ingestion socket;
- directory ingestion for notes, CSVs, and emails.

What is still needed:

- ~~a `moonraker` connector~~ — **done** (`src/moonraker.rs`): polls
  `printer.objects.query` over Moonraker's JSON-RPC endpoint.
- ~~mapping Moonraker fields to a stable printer schema~~ — **done**: the
  adapter emits a normalized `OperationalEvent` with `machine_id`, job subject,
  print/printer state, progress, message, and observation time.
- ~~a small CLI command or sidecar~~ — **done** (`src/moonraker_connect.rs`):

  ```bash
  physis-pro connect moonraker --url http://printer.local:7125 \
      --machine-id voron-01 --interval 10 \
      --state-file ~/.physis/moonraker-voron-01.json \
      --post-to http://127.0.0.1:19876 --user shopfloor
  ```

  Emits one JSON line per event (composes with `jq`, a file, or a pipe) and,
  with `--post-to`, registers each event into physis as a coherence node
  carrying an asserted verdict — so a printer warning is recallable by meaning
  and visible to the dream loop.
- ~~durable event storage and deduplication~~ — **done**, with a caveat worth
  recording: `event_key` cannot serve as the dedupe key for a poller, because
  `OperationalEvent::rekey` hashes `observed_at` and a poll stamps that with the
  wall clock. Two polls of an idle printer therefore produce two different keys
  for the same reality. The connector instead compares a `StateFingerprint`
  (status, subject, print state, message) and persists it via `--state-file`, so
  a restart does not re-report what was already reported. Continuously-moving
  values like progress are deliberately excluded — a print advancing 41% → 42%
  is not an event, but `printing` → `paused` is.

Still open for the live demo:

- subscribing to Moonraker's WebSocket instead of polling, for sub-second
  pause/resume detection;
- attaching the printer's own job history so a restart can backfill events it
  missed while the connector was down.

The first implementation should be a separate sidecar or adapter, not a change
to the core classifier. It can emit the existing telemetry JSON or MQTT topic:

```json
{
  "machine_id": "voron-01",
  "status": "warning",
  "temperature": 78.5,
  "raw": {"bed_target": 60, "print_progress": 0.42, "job_state": "paused"}
}
```

Do not market this as predictive maintenance for 3D printers. Market it as an
observable, explainable workflow demo and use it to validate ingestion,
timeline-building, anomaly explanation, and feedback.

## Personal workflows

Physis can model workflows if each step becomes an event or document with:

- actor or system;
- timestamp and duration;
- input/output or linked artifact;
- status (`success`, `inert`, `failure`);
- optional relation to a goal, machine, customer, or task.

The current engine has goals, PDCA, graph edges, file watching, and coherence
assertions, but it does not yet provide a general workflow event schema. Add
that schema before building many bespoke integrations. A useful first view is a
timeline plus “stuck handoffs”: items created but not completed, repeated
failures, and work waiting on another person.

## System monitoring

Technically, Physis could consume process and application telemetry: active
window/application, process start/stop, CPU/memory, file or network events, and
manual notes. The current repository does not provide this connector.

Build it as an explicit, opt-in local collector with:

- coarse application names rather than keystrokes or screen contents;
- configurable allow/deny lists and a pause switch;
- aggregation (for example, minutes per application per project);
- local-only storage by default;
- deletion and export controls;
- a clear distinction between observation and judgement.

The product use case is “where does my work time and context switching go?”
not employee surveillance. Do not collect keystrokes, screenshots, message
content, or hidden telemetry. For workplace deployment, obtain informed consent
and involve HR/legal/security before enabling any monitoring.

## Email chains

`.eml` files already work with the `email` feature, including sender, subject,
body, and a heuristic thread ID. That is enough for a local demo: export a
mailbox or place selected messages in a watched directory.

Production connectors are still required for IMAP, Microsoft Graph, and Gmail.
Implement them as explicit mailbox scopes with:

- OAuth or app-password authentication handled outside the core;
- incremental sync using provider cursors/IDs;
- normalized `thread_id`, `message_id`, sender, recipients, timestamps, and
  quoted-reply removal;
- redaction and retention policies;
- tenant/user ownership and audit logs.

The useful product output is not “classify every email.” It is a thread-level
process view: unresolved requests, promised dates, approvals waiting on people,
and contradictions between messages and machine/production evidence.

## Product boundary

The demo should prove this loop:

```text
telemetry + files + email
          -> normalized events/documents
          -> typed timeline and graph
          -> anomaly or process friction
          -> evidence-backed recommendation
          -> human verdict
          -> measurable outcome
```

The graph, embeddings, semiotic grid, and dream proposals are implementation
advantages. The customer-facing promise is faster diagnosis and fewer lost
handoffs, with the customer retaining control over data and actions.

## Next build steps

1. ~~Add a normalized `OperationalEvent` model with provenance and timestamps.~~ Done.
2. ~~Expand the Moonraker adapter into the live printer demo path~~ — done via
   `physis-pro connect moonraker`; keep the replayable JSON fixture in sync with
   the real query shape.
3. Wire the timeline/incident view into the web surface and join telemetry,
   documents, and email thread IDs.
4. Add explicit approve/reject/inert feedback and outcome measurement.
5. Keep system monitoring as a separate opt-in collector with strict privacy
   defaults.
6. Add IMAP/Graph/Gmail only after the local `.eml` workflow proves useful.
