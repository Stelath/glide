use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use anyhow::{Context, Result};

use super::types::{BenchmarkReport, CompareFailure, CompareOptions, CompareResult, CompareRow};

pub fn compare_report_files(options: &CompareOptions) -> Result<CompareResult> {
    let baseline = read_report(&options.baseline)?;
    let candidate = read_report(&options.candidate)?;
    Ok(compare_reports(
        &baseline,
        &candidate,
        options.fail_threshold_percent,
    ))
}

fn read_report(path: &Path) -> Result<BenchmarkReport> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read benchmark report {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn compare_reports(
    baseline: &BenchmarkReport,
    candidate: &BenchmarkReport,
    fail_threshold_percent: f64,
) -> CompareResult {
    let baseline_by_phase = baseline
        .summary
        .iter()
        .map(|summary| (summary.phase.as_str(), summary))
        .collect::<BTreeMap<_, _>>();
    let candidate_by_phase = candidate
        .summary
        .iter()
        .map(|summary| (summary.phase.as_str(), summary))
        .collect::<BTreeMap<_, _>>();

    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for phase in baseline_by_phase
        .keys()
        .chain(candidate_by_phase.keys())
        .copied()
        .collect::<BTreeSet<_>>()
    {
        let baseline = baseline_by_phase.get(phase);
        let candidate = candidate_by_phase.get(phase);
        let median_delta_percent = baseline
            .zip(candidate)
            .and_then(|(base, cand)| percent_delta(base.median_ms, cand.median_ms));
        let p95_delta_percent = baseline
            .zip(candidate)
            .and_then(|(base, cand)| percent_delta(base.p95_ms, cand.p95_ms));

        if let Some(delta) = median_delta_percent
            && delta > fail_threshold_percent
        {
            failures.push(CompareFailure {
                phase: phase.to_string(),
                metric: "median".to_string(),
                delta_percent: delta,
            });
        }
        if let Some(delta) = p95_delta_percent
            && delta > fail_threshold_percent
        {
            failures.push(CompareFailure {
                phase: phase.to_string(),
                metric: "p95".to_string(),
                delta_percent: delta,
            });
        }

        rows.push(CompareRow {
            phase: phase.to_string(),
            median_delta_percent,
            p95_delta_percent,
        });
    }

    CompareResult { rows, failures }
}

fn percent_delta(baseline: f64, candidate: f64) -> Option<f64> {
    (baseline > 0.0).then(|| ((candidate - baseline) / baseline) * 100.0)
}
