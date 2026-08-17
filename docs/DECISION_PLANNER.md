# Decision Planner product brief

Physis Decision Planner is the counterfactual layer of the product. It answers:

> If we change how the organization names and groups things, what changes in
> the data, reports, queues, and decisions?

The planner works on a snapshot and produces an impact diff. It does not alter
the live classifier until an authorized user promotes the scenario.

## Implemented MVP slice

The first productized slice is available through the HTTP API:

- `POST /api/v1/planner/simulate` accepts a candidate ontology plus a bounded
  text corpus and returns changed/unchanged classifications, confidence
  deltas, low-confidence items, and transition counts.
- `POST /api/v1/admin/review` groups supplied document metadata into a review
  queue and duplicate groups. It is deliberately stateless until persistence,
  tenant isolation, and an audit ledger are added.

The candidate ontology is built and discarded inside the request, so the
simulation cannot mutate the live classifier. The current slice does not yet
persist scenarios, infer dated signifier shifts, or promote/reject a scenario;
those remain the next paid-pilot features.

The first public demonstration will run on procurement data and show an
ontology replacement plus an inferred shift in the meaning of “supplier
delay.” The commercial page for this capability is the Physis landing page at
`https://praxisweb.xyz/`; the technical walkthrough remains at `/physis`.
