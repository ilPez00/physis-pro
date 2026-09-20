# MS-003 Measurement Notes

## Raw Command Outputs

### Transform Tests (lib)
```
running 4 tests
test transform::tests::absent_relation_yields_incoherence ... ok
test transform::tests::invalid_analogy_rejected ... ok
test transform::tests::substitution_preserves_invariants ... ok
test transform::tests::reciprocal_resource_exchange_abstraction_transfers ... ok
test result: ok. 4 passed
```

### Becoming Tests (lib)
```
running 12 tests
test becoming::tests::broad_use_of_one_sense_is_stable_under_labels ... ok
test becoming::tests::a_lone_outlier_is_a_mistake_but_a_repeated_one_is_a_variation ... ok
test becoming::tests::interleaved_senses_are_a_split ... ok
test becoming::tests::labeled_verdicts_are_reproducible ... ok
test becoming::tests::one_dominant_label_is_stable_even_with_stragglers ... ok
test becoming::tests::runs_z_is_negative_when_clumped_and_positive_when_alternating ... ok
test becoming::tests::labeled_time_separated_is_a_becoming ... ok
test becoming::tests::one_sense_is_stable_however_it_is_ordered ... ok
test becoming::tests::labeled_interleaved_is_a_split ... ok
test becoming::tests::too_few_occurrences_says_so_rather_than_guessing ... ok
test becoming::tests::time_separated_senses_are_a_becoming ... ok
test becoming::tests::verdicts_are_reproducible ... ok
test result: ok. 12 passed
```

### Transform Bench Tests
```
running 4 tests
test attractive_invalid_rejected ... ok
test counterfactual_absence_lists_dependents ... ok
test symbolic_rule_baseline_fails_transfer ... ok
test transfer_beats_similarity ... ok
test result: ok. 4 passed
```

### Transform Callers in src/
```
(no output) → ZERO production callers
```

### Becoming Callers in src/
```
src/chain.rs:418-442 → Single integration point
```

### Chain.rs Becoming Integration (lines 418-442)
```rust
let mut drifts = Vec::new();
for family in map.repeats.iter().take(8) {
    let occs: Vec<crate::becoming::Occurrence> = docs
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| family.instances.iter().any(|i| i.path == *name))
        .map(|(i, (_, body))| crate::becoming::Occurrence {
            at: i as i64,
            context: embedder.embed(body),
        })
        .collect();
    if occs.len() < 6 {
        continue;
    }
    let v = crate::becoming::classify(&occs);
    if !matches!(v.trajectory, crate::becoming::Trajectory::Stable) {
        drifts.push(Drift { ... });
    }
}
```

### RelationType Variants (from enum definition)
1. Supports
2. Contradicts
3. Causes
4. Influences
5. Precedes
6. Follows
7. DependsOn
8. Requires
9. Produces
10. Measures
11. Observes
12. Explains
13. Predicts
14. Invalidates
15. DerivedFrom

### Predicate Variants (from enum definition)
1. Exchanges
2. Causes
3. Requires
4. Produces
5. Supports
6. Contradicts
7. Precedes
8. Observes
9. Custom(String)

### Key Thresholds (verified from source)
- `ACCEPT_FLOOR = 0.5` (transform.rs:386)
- Cosine separation threshold = 0.15 (becoming.rs:215, 1.0 - cosine)
- `runs_z` threshold = -1.96 (becoming.rs:236, Wald-Wolfowitz)
- Min occurrences for becoming = 6 (chain.rs:430, becoming.rs:188)
- Max families processed = 8 (chain.rs:419)

### File Line Counts
- transform.rs: 759 lines
- becoming.rs: 507 lines
- relation.rs: 171 lines
- chain.rs: 622 lines

## Observations

1. **Transform is completely isolated from production** — only tests/examples call it
2. **Becoming has exactly one production integration** — chain.rs repeat-family drift detection
3. **RelationType is completely disconnected** from both Transform and Becoming measurement pipelines
4. **Predicate ↔ RelationType overlap**: 7 shared concept names (Causes, Requires, Produces, Supports, Contradicts, Precedes, Observes) but no structural connection
5. **No relation survival measurement exists** — would require ground-truth corpus of paraphrased/renamed documents with known relation labels
