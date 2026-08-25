# TESTING.md — Human Testing Guide (Phase 6 + 9 surfacing)

Audience: you, testing your own engine by hand. Everything below runs against
the local build in `target/debug/` and needs **no network** except the one
crates.io verification already done.

---

## 0. One-time setup (~5 min)

The binary is licence-gated. Mint a local dev licence with the vendor tool
(already built):

```bash
cd /home/gio/dev/physis-pro

# 1. Mint a 1-year all-features licence from the vendor keypair
./target/debug/physis-license sign \
    --key ~/.physis-vendor/signing.key \
    --org "Gio Dev" --seats 1 --features '*' \
    --exp 2027-08-25 > .physis/license.key

# 2. Sanity check: doctor must pass
CARGO_TARGET_DIR=target ./target/debug/physis-pro doctor | head -20
```

Expected: `doctor` reports ok for config / data dir / embedder / licence.
If the embedder self-test complains, that's fine on this box (RP fallback) —
it's a warning, not a failure.

---

## 1. Review Desk — `POST /api/v1/admin/desk` (PLAN 6.1)

Tests: exceptions queue over a folder of documents.

```bash
# Start the web server (dev mode: no auth token needed on loopback)
PHYSIS_DEV=1 CARGO_TARGET_DIR=target ./target/debug/physis-pro-web &
# wait ~3s for "listening", then:

mkdir -p /tmp/desktest
printf 'Invoice No. INV-777\nVendor: ACME SpA\nDate: 2026-08-01\nTotal: 120.00 EUR' > /tmp/desktest/a.pdf.txt
cp /tmp/desktest/a.pdf.txt /tmp/desktest/b.pdf.txt          # exact duplicate
printf 'random unclassifiable words zzzz qqqvvv' > /tmp/desktest/c.xyz

curl -s -X POST localhost:19876/api/v1/admin/desk \
     -H 'content-type: application/json' \
     -d '{"dir": "/tmp/desktest"}' | python3 -m json.tool | head -40
```

**PASS if**: JSON has `report.exceptions` containing a `duplicate` row (for
a.pdf.txt AND b.pdf.txt) and probably a `low_confidence` row for c.xyz;
`markdown` field renders as a readable table. Re-running gives identical output.

**FAIL**: any 500, or duplicates not detected, or c.xyz silently absent.

Kill server when done: `kill %1`

---

## 2. Batch CSV — `POST /api/v1/admin/batch` (PLAN 6.2)

```bash
PHYSIS_DEV=1 CARGO_TARGET_DIR=target ./target/debug/physis-pro-web &

cat > /tmp/batch.json <<'EOF'
{"csv_text":"vendor,doc_number,date,amount_eur\nACME,A-1,2026-01-31,\"1.234,56\"\nBeta,B-2,,88\nGamma,C-3,31/01/2026,10\n"}
EOF

curl -s -X POST localhost:19876/api/v1/admin/batch \
     -H 'content-type: application/json' -d @/tmp/batch.json | python3 -m json.tool
```

**PASS if**:
- Row A-1: `status:"ok"` and amount = 1234.56 (European decimal parsed)
- Row B-2: `status:"missing_field"`, error names `doc_number`
- Row C-3: `status:"bad_date"` (dd/mm/yyyy rejected)
- `valid` = 1, `export_csv` contains a `status` column and all 3 rows

Also test feature-off path (optional): rebuild without `ingest` → same call
returns **501** with a helpful message instead of a crash.

---

## 3. Weekly Management Pack — `POST /api/v1/admin/pack` (PLAN 6.4)

```bash
curl -s -X POST localhost:19876/api/v1/admin/pack \
     -H 'content-type: application/json' \
     -d '{"week_label":"2026-W35","plant":"Verona","notes":["Compressor bearing replaced"]}' \
     | python3 -c 'import json,sys; print(json.load(sys.stdin)["markdown"])'
```

**PASS if** the markdown prints: headline table (Documents processed 0),
"No invoices in scope this week.", notes bullet. Output must be byte-identical
across two identical calls (diff them).

Chain test (desk → pack): run test 1 first, then feed its exception count in
by hand — pack is intentionally caller-fed, no hidden coupling.

---

## 4. Communities — studio tab + endpoint (PLAN 9.4b)

```bash
cd physis-core && CARGO_TARGET_DIR=target cargo build --features studio && cd ..
(CARGO_TARGET_DIR=target target/debug/physis-core studio --port 19877 &)
sleep 3
curl -s localhost:19877/api/v1/communities | python3 -m json.tool | head -30
```

**PASS if**: JSON lists communities with `members` (labels) and `cohesion`
(0..1). On the persisted QA store expect many singletons (cohesion 1.0) —
that's correct: those nodes have no typed edges yet.

Browser check: open http://localhost:19877/ → click **Communities** tab →
panels sorted by size render; Reload button works.

Kill: `kill %1`

---

## 5. Wiki generation — outline/prose/tour (PLAN 9.5)

Via CLI (no server needed):

```bash
mkdir -p /tmp/wikitest && printf 'We chose sled for persistence.\nSecond doc about pumps.\n' > /tmp/wikitest/doc1.md
printf 'Pump maintenance required monthly.\n' > /tmp/wikitest/doc2.md
CARGO_TARGET_DIR=target ./target/debug/physis-pro watch --once /tmp/wikitest  # register nodes
```

Then via HTTP (server running from §1):
```bash
curl -s -X POST localhost:19876/api/v1/wiki/generate \
     -H 'content-type: application/json' -d '{}' | python3 -m json.tool | head -40
curl -s localhost:19876/api/v1/wiki/tour | python3 -m json.tool | head -20
```

**PASS if**: `/wiki/generate` returns pages covering every node you ingested
(nothing dropped), each page has title/scope/members markdown; `/wiki/tour`
returns an ordered list. LLM-less run uses deterministic fallback titles
("... cluster (N nodes)") — that's expected without a provider configured.

---

## 6. Session decision mining — `decisions-mine` (PLAN 9.3)

Real transcripts already exist on this machine:

```bash
CARGO_TARGET_DIR=target ./target/debug/physis-pro decisions-mine \
    --dir ~/.claude/projects/-home-gio-dev-physis-pro 2>/dev/null | head -10
```

**PASS if**: zero-to-N JSON lines, each with `decision`, `verification`
("grounded"/"partial"), no panics on weird lines. Empty output is legitimate
(cue sentences are rare in real sessions); a crash is not.

---

## 7. Full-suite regression (before you sign off)

```bash
CARGO_TARGET_DIR=target cargo test 2>&1 | grep -E 'test result' | sort | uniq -c
```

**PASS if**: every line says `ok`; totals ≈ 626 lib + integration suites,
0 failed anywhere.

---

## Known-good environment notes

- `PHYSIS_DEV=1` is REQUIRED for token-less loopback web runs (refuses otherwise by design).
- The web server (`physis-pro-web`) binds **127.0.0.1:19876** by default —
  override with `PHYSIS_HOST` / `PHYSIS_PORT`. Studio binds the port you pass
  (`--port 19877` below).
- `.physis/` holds sled state — delete `state.sled` to reset the holarchy
  between test rounds (you'll lose ingested nodes).
- Sibling agent may hold the cargo build lock; if a build hangs at "Blocking
  waiting for file lock", wait or ask them to finish.
