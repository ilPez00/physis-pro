# Classifier productization plan

The classifier is a stable 5 × 14 coordinate system, not a claim that every
observation belongs somewhere. The implementation now treats the grid as a
contract and the ontology as an expandable evidence layer.

## Implemented MVP

1. The canonical domains (`HEAL`, `CONSTRUCT`, `FABRICATE`, `BOND`, `STUDY`)
   and fourteen modes are explicit. Missing cells receive a neutral fallback
   anchor; the richer `mode_anchors` ontology remains the preferred evidence.
2. Sector, client, and project overlays can lift a matching cell in the active
   context. The response reports the context and number of active overlay
   definitions. Overlay influence is bounded and auditable.
3. `/api/v1/classify` returns `assessment` with model provenance, semantic
   status, top score, margin, coverage, confidence, verdict, abstention, and a
   plain-language reason. `strong`, `review`, `unmapped`, and `degraded` are
   deliberately distinct outcomes.
4. A degraded or weak result abstains rather than presenting a random or weak
   match as fact. The existing public embedder endpoint remains the operational
   source of truth for model health.
5. Weak semantic results include a review-only discovery suggestion: proposed
   name, hints, nearest coordinate, and measured coverage. Nothing is installed
   automatically.

## Acceptance checks

- The canonical grid always has 70 coordinates.
- Overlay ranking changes are bounded and disappear when context is empty.
- Random-projection mode always reports `degraded` and abstains.
- Low-coverage semantic text reports `unmapped` and a discovery suggestion.
- Strong semantic text reports `strong` only when coverage and separation both
  support the result.

## Next evidence loop

Collect labelled customer observations by sector, measure top-1 accuracy and
abstention precision, then promote repeated discovery suggestions into the
appropriate overlay or specialist ontology. Do not add universal cells merely
to make the top-k list look fuller.
