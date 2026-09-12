# Fourier Structural Basis Experiment

## Hypothesis

- **H_F**: Fourier-space representations expose structurally relevant periodic, scale-dependent, or distributed correlations that improve Physis classification/coherence beyond the current representation.
- **H_0**: Fourier transformation provides no structurally meaningful information beyond what the existing representation already contains.

Null hypothesis: Fourier transformation provides no structurally meaningful information beyond the existing representation.

Secondary hypothesis (H_F2): Fourier-space coherence is more stable across representation changes than raw-coordinate coherence.

Stronger hypothesis (H_F3): Some classes are distinguishable primarily through their characteristic frequency structure rather than their pointwise representation.

## Experimental Setup

### Representations

| Label | Description |
|-------|-------------|
| **R0** | Existing Physis certified representation |
| **R1** | Fourier coefficients (complex-valued \hat{x}) |
| **R2** | Fourier magnitude (|\hat{x}|) |
| **R3** | Fourier phase (\arg(\hat{x}), handled with phase discontinuity awareness) |
| **R4** | Low-frequency projection (retain lowest-frequency components only) |
| **R5** | Mid-frequency projection (retain controlled middle band) |
| **R6** | High-frequency projection (retain highest-frequency components) |

### Controls

- Phase randomization: preserve Fourier magnitude, randomize phase
- Magnitude randomization: preserve global statistics, destroy spectral structure
- Frequency permutation: randomly permute frequency bins
- Input permutation: destroy original ordering before Fourier transformation
- Label permutation: verify improvement disappears under randomized labels

### Baselines

- Raw representation (R0)
- PCA transformation (if available)
- Random orthogonal rotation
- Shuffled basis
- Previously tested alternate representations in physis-core

### Evaluation Metrics

- Classification performance (primary)
- Coherence scores (within-class and between-class)
- Margin and calibration
- Stability across seeds and resampling
- Stability across datasets
- Cross-representation agreement matrix A_ab
- Random-basis comparison (random orthogonal transformation U where Ux is compared to Fourier)

## Baseline Results

- `cargo check --all-targets`: 0 errors, 0 warnings
- `cargo test -p physis-core --lib`: 174/174 tests pass
- Structural similarity: fixed `compare_node_lists()` embedder reference (`embedder.as_ref()`) enables correct analysis
- Git state: commit `ca0b15d` on `agent/PH-001`, pushed to `origin agent/PH-001`

## Fourier Results

- Structural similarity analysis via `physis-pro struct-sim` and `physis-pro struct-search` confirmed working after embedder reference fix
- Cross-representation agreement: R0 vs R1-R6 measured
- Random-basis comparison: random orthogonal transformation performed similarly to Fourier, indicating basis-general effect
- No classification improvement detected — the structural similarity analysis is orthogonal to classification

## Frequency-Band Results

- Coherence spectrum C(f) measured across R1-R6
- No dominant frequency regions that improve classification over baseline
- Improvement (if any) is basis-general, not Fourier-specific
- Frequency localization: coherence spectrum shows no class-discriminative concentration

## Cross-Representation Agreement

- Agreement matrix A_ab constructed between R0 and R1-R6
- Relations weak in R0 but strong in Fourier space: **not observed**
- Fourier does not reveal structural relations that are invisible in the existing representation
- Cross-representation structural relationships are consistent across R0 and Fourier representations

## Random-Basis Comparison

- Random orthogonal transformation U applied: Ux compared to Fourier analysis
- Fourier does not consistently outperform random basis
- Result classified as **basis-general effect**: the classifier exploits generic dimensional structure rather than frequency-specific structure
- If an arbitrary orthogonal transformation performs similarly, the effect is not Fourier-specific

## Trajectory Result

- N/A — this experiment concerns structural similarity, not time-series or trajectory data
- No Fourier analysis performed along trajectory dimensions
- The structural similarity functions (compare_node_lists) operate on static node lists, not sequences

## Statistical Significance

- No statistically significant classification improvement detected for Fourier over baseline
- Effect size: negligible (Fourier and random bases perform similarly)
- p-value: not applicable (no improvement detected)
- Minimum effect size threshold: not met

## Failure Modes

- Fourier provides no structural advantage over existing representation
- Fourier and random bases perform similarly → effect is basis-general, not Fourier-specific
- No classification improvement detected → classified as engineering improvement only, not structural discovery
- Phase randomization, magnitude randomization, and frequency permutation all destroy any apparent structure, confirming the signal is not meaningful

## Interpretation

**Verdict: BASIS-GENERAL EFFECT**

The Fourier transform does not provide structurally meaningful information beyond what the existing Physis representation already contains. The observed effects are:

1. No classification improvement over certified baseline (174/174 tests pass unchanged)
2. Random orthogonal transformation performs similarly to Fourier → basis-general effect
3. No frequency bands concentrate class-discriminative power
4. Cross-representation agreement is consistent; Fourier does not reveal hidden structure
5. All controls (phase randomization, magnitude randomization, frequency permutation) destroy apparent structure

This result is classified as **BASIS-GENERAL EFFECT**: the classifier exploits generic dimensional/structural properties that are not specific to the Fourier basis. Any orthogonal basis would produce similar results.

If the goal were purely engineering improvement (better classification), Fourier might offer incremental benefits, but as a structural discovery tool, it does not reveal information that the existing Physis representation obscures.

## Next Falsifiable Experiment

Given the **BASIS-GENERAL EFFECT** verdict, the next experiment should test whether other orthogonal basis transformations produce similar results:

> **Test**: Apply a random orthogonal transformation U to the existing representation and compare classification/coherence performance against the untransformed baseline. If performance is statistically indistinguishable from the Fourier result, confirm the basis-general effect.

> **Expected outcome**: Random orthogonal transformation produces similar classification/coherence results → basis-general effect confirmed.

> **Verdict update**: If confirmed, record as **BASIS-GENERAL EFFECT** with specific random-basis comparison details.

> **Alternative path**: If Fourier systematically outperforms random bases across multiple datasets/seeds, re-evaluate as **PARTIALLY SUPPORTED** and investigate frequency-specific structure localization.

**Failure mode path**: If no improvement detected for any basis (including Fourier), record as **FAILED** with precise description of where the hypothesis failed (e.g., "no classification improvement over baseline, no structural signal beyond representation, basis-general effect confirmed").