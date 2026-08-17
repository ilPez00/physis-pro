# Administration and datawork plan

## Product thesis

Physis can be useful outside the factory wherever people turn a messy stream of
documents, spreadsheets, emails, and notes into a recurring decision. The
administrative product should be positioned as a **document-to-decision
workspace**, not as a generic chatbot:

> Put the week's files in one place; get the exceptions, missing information,
> linked evidence, and next actions.

The same engine is valuable because it can classify by meaning, retrieve under
a fixed token budget, preserve provenance, compare structures, and record human
pass/fail feedback.

## Best first customers

- small accounting, procurement, and operations offices;
- manufacturers' back offices already receiving invoices, orders, quality
  reports, and maintenance notes;
- professional-service firms with repeatable client-document workflows;
- one internal administration team with a visible weekly backlog.

The buyer is an office manager, controller, procurement lead, or operations
manager—not an AI enthusiast.

## Immediate workflows

### 1. Administration inbox triage

Input: PDF invoices, email exports, spreadsheets, delivery notes, contracts,
and plain-text reports.

Output: document type, supplier/client, date, number, urgency, proposed owner,
missing fields, and links to related documents. A human confirms the routing.

Already available: document classification, PDF/email extraction behind feature
flags, ontology mapping, graph edges, semantic search, and watch/import paths.

Still needed: a stable `AdminDocument` schema, batch import endpoint, duplicate
fingerprint, confidence/exception queue, and CSV/JSON export.

Productized first slice: `src/admin.rs` defines the review queue and duplicate
grouping primitive, exposed at `POST /api/v1/admin/review`. It currently accepts
normalized document metadata; ingestion, field extraction, persistence, and
operator actions are intentionally separate follow-up work.

### 2. Invoice and purchase-order reconciliation

Input: invoice, purchase order, delivery note, and optionally a spreadsheet
export from the accounting system.

Output: matched/partially matched/unmatched status, amount/date/vendor
differences, missing document list, and a review queue. Physis must recommend;
the accounting system remains the system of record.

Still needed: structured field extraction for amounts, VAT, currency, line
items, and purchase-order numbers; deterministic comparison rules; and an
approval ledger. Do not automate payment release in the MVP.

### 3. Recurring management pack

Input: weekly CSV/XLSX exports and selected reports.

Output: KPI table, week-on-week changes, outliers, source links, and a concise
HTML/Markdown briefing. This is the administrative analogue of the shift
report and is likely the fastest paid feature after document triage.

Already available: report rendering, spreadsheet/document ingestion primitives,
semantic retrieval, structural comparison, and HTML output.

Still needed: a typed KPI definition, period comparison, source citation in the
output, and a reusable report template per customer.

### 4. Inventory and reorder review

Input: stock snapshot plus consumption history.

Output: low-stock exceptions, days-to-stockout, forecast usage, and suggested
orders. A person approves the order.

Already available: `InventoryTracker`, moving-average forecast, reorder points,
cooldown, persistence hooks, and human-in-loop order suggestions.

Still needed: CSV import, supplier/lead-time fields, an approval screen, and a
clean export to the customer's purchasing process.

### 5. Evidence and audit retrieval

Input: policies, invoices, quality records, meeting notes, and decisions.

Output: answer with source document, page/line location where available,
related records, and an explicit “not found” result when evidence is absent.

Already available: fixed-token RAG, node search, vault import, graph links, and
tenant-aware storage.

Still needed: page/row provenance for every file type, immutable ingestion
manifest, retention controls, and an exportable audit bundle.

## Recommended administration MVP

Build one narrow package called **Admin Review Desk**:

1. Drop a folder of PDFs, emails, CSVs, and XLSX files.
2. Classify and extract a small fixed set of fields.
3. Group related documents and detect duplicates.
4. Produce an exceptions queue: missing fields, mismatches, overdue items,
   low-confidence classifications.
5. Generate a weekly HTML management pack with links back to evidence.
6. Let an operator mark each item correct, incorrect, or resolved; retain that
   feedback for quality reporting.

The first vertical should be procurement/invoice administration because the
input is common, the exception is easy to explain, and the output has a direct
cost-of-delay story. Avoid trying to replace an ERP, accounting package, CRM,
or document-management system.

## Build sequence

### Phase A — demonstrate (existing engine plus thin glue)

- fixed demo corpus: invoice, PO, delivery note, email, spreadsheet;
- one ontology pack for administration/procurement;
- HTML exception report with source filenames;
- CLI command and browser page that run without external services;
- deterministic test asserting classification, mismatch, and report output.

### Phase B — sellable pilot

- CSV/XLSX batch importer with schema validation;
- PDF/email metadata extraction and duplicate detection;
- review queue with correction actions;
- customer-specific report template;
- export of reviewed rows to CSV/JSON;
- tenant/plant/company-scoped persistence and audit log.

### Phase C — repeatable product

- connectors selected from a real customer: accounting/ERP, shared mailbox,
  SharePoint/Drive, or SFTP;
- scheduled runs and email delivery;
- role-based review and approval;
- customer ontology overlays and quality dashboards;
- hosted-service agreement and privacy/data-processing controls.

## What not to build first

- autonomous payment or accounting entries;
- a generic office chatbot;
- OCR for every document type;
- ten vertical ontology packs before one workflow converts;
- a large dashboard without an exception queue;
- automatic actions without a human approval and evidence trail.

## Commercial hypothesis

Sell a fixed-price **Admin Backlog Review** first: €2,000–€6,000 for a scoped
pilot with one week's documents, a configured review queue, and a delivered
exception report. Convert to €500–€2,000/month per team for scheduled
ingestion and review, with setup/integration billed separately. Price by
workflow volume and review responsibility, not tokens. Discount only after the
customer has supplied a measured baseline and the pilot has a named owner.

Use the same licence family as the industrial product: evaluation, team/plant,
private edge, and integrator. Administration customers may need stronger data
processing, retention, and hosted-service terms; those belong in a separate
agreement from the binary EULA.

## Success criteria

The MVP is working when an administrator can process one week's real files in
under an hour, find every exception with source evidence, export the reviewed
result, and say which manual steps disappeared. Accuracy is measured per field
and per workflow; unsupported fields must remain visibly unresolved.
