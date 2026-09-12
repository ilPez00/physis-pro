use crate::scanner::scan_project;
use crate::pack::{pack_cached, EmbedCache};
use crate::embed::RandomProjectionEmbedder;
use std::path::Path;
use std::time::Instant;
use anyhow::Result;

/// Result returned by the estimate command.
#[derive(Debug, Clone)]
pub struct EstimateResult {
    /// Tokens counted before packing (raw file text).
    pub baseline_tokens: usize,
    /// Tokens after packing within the budget.
    pub packed_tokens: usize,
    /// Percentage of tokens saved relative to the baseline.
    pub saved_percent: f64,
    /// Elapsed time for the filesystem scan, in seconds.
    pub scan_elapsed_secs: f64,
    /// Estimated model‑generation time for the packed context, in seconds.
    pub model_time_est_secs: f64,
}

/// Compute the numeric estimate from two token counts.
pub fn calculate_estimate(baseline: usize, packed: usize) -> EstimateResult {
    let saved = if baseline > 0 {
        (baseline as f64 - packed as f64) / baseline as f64 * 100.0
    } else {
        0.0
    };
    EstimateResult {
        baseline_tokens: baseline,
        packed_tokens: packed,
        saved_percent: saved,
        scan_elapsed_secs: 0.0,
        model_time_est_secs: 0.0,
    }
}

/// Scan the given directory and pack its context, returning an estimate.
pub fn estimate_project(dir: &Path) -> Result<EstimateResult> {
    let scan_start = Instant::now();
    let _files = scan_project(dir, None);
    let scan_elapsed = scan_start.elapsed().as_secs_f64();

    // Minimal pack run to obtain a packed‑token count.
    let query = "context";
    let budget = 4000usize;
    let top_k = 50usize;
    let diversity = 0.3f32;
    let embedder = RandomProjectionEmbedder::new(768);
    let embedder_kind = "random-projection";
    let mut cache = EmbedCache::load(Path::new("."));

    let pack_result = pack_cached(
        dir,
        query,
        budget,
        top_k,
        diversity,
        &embedder,
        embedder_kind,
        &mut cache,
    );

    let packed_tokens = pack_result.tokens as usize;

    // Rough model‑generation estimate: 1 token ≈ 0.02 s on a typical GPU‑less CPU.
    let model_time_est = (packed_tokens as f64) * 0.02;

    Ok(EstimateResult {
        baseline_tokens: packed_tokens, // placeholder; real baseline would be total file tokens
        packed_tokens,
        saved_percent: 0.0, // placeholder; real saved% computed from baseline vs packed
        scan_elapsed_secs: scan_elapsed,
        model_time_est_secs: model_time_est,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_estimate_full_save() {
        let r = calculate_estimate(1000, 800);
        assert!((r.saved_percent - 20.0).abs() < 0.001);
        assert_eq!(r.baseline_tokens, 1000);
        assert_eq!(r.packed_tokens, 800);
    }

    #[test]
    fn test_calculate_estimate_zero_baseline() {
        let r = calculate_estimate(0, 100);
        assert_eq!(r.saved_percent, 0.0);
        assert_eq!(r.baseline_tokens, 0);
        assert_eq!(r.packed_tokens, 100);
    }

    #[test]
    fn test_calculate_estimate_baseline_zero_packed_zero() {
        let r = calculate_estimate(0, 0);
        assert_eq!(r.saved_percent, 0.0);
    }
}