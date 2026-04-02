# Verification Record: Snapshot-Owned Reverse Import Indexes And Hot-Path Perf Baselines

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the final hot-path risk reduction covering:

- snapshot-owned reverse import lookup in ingest
- removal of graph-local reverse import adjacency rebuild
- explicit hot-path perf thresholds for `blast-radius` and `get-brief`
- repo-native `cargo xtask perf` measurement surface

## Intent / Spec References

- Intent: [Snapshot-owned reverse import indexes and hot-path perf baselines intent](d:/RepoBrainOS/plans/2026-03-30-snapshot-owned-reverse-import-indexes-and-hot-path-perf-baselines-intent.md)
- Spec: [Snapshot-owned reverse import indexes and hot-path perf baselines spec](d:/RepoBrainOS/plans/2026-03-30-snapshot-owned-reverse-import-indexes-and-hot-path-perf-baselines-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-ingest`
- `cargo test -p repobrain-graph`
- `cargo test -p repobrain-broker`
- `cargo xtask perf`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-ingest` now owns reverse import lookup as a derived snapshot index, and `repobrain-graph` consumes that maintained lookup instead of rebuilding a local reverse map.

`cargo xtask perf` now builds a stable synthetic Rust fixture, scans it once, warms the hot paths, measures repeated `blast-radius` and `get-brief` runs, prints p50/p95/mean/max, and enforces p95 thresholds.

This closes the previous residual risk where reverse import lookup ownership and hot-path latency baselines were still missing.

## Pass / Fail Against Expectations

Pass.

This slice moves the remaining reverse-neighbor ownership into ingest and gives the repo a real hot-path perf command without forcing benchmark theater into every default validation run.

## Performance / Complexity Validation

- Workload exercised:
  synthetic Rust fixture with one anchor symbol and many reverse importers, plus full repo-native validation
- Measured:
  `cargo xtask perf` reported explicit p50/p95/mean/max and stayed within the configured p95 thresholds for both `blast-radius` and `get-brief`
- Inferred:
  moving reverse import lookup into the snapshot removes graph-local reverse-adjacency rebuild cost and keeps graph as a pure consumer of maintained indexes
- Why no benchmark was needed, if applicable:
  not applicable; this slice added an explicit measurement surface because the remaining risk was specifically the lack of hot-path thresholds

## Residual Risks

- current perf thresholds are synthetic-workload baselines, not full live-repo latency guarantees
- future commands like `explain-flow` still need their own measurement surface
- additional graph edge families may still need snapshot-owned reverse indexes later

## Related

- Research: [Snapshot-owned reverse import indexes and hot-path perf baselines](d:/RepoBrainOS/research/2026-03-30-snapshot-owned-reverse-import-indexes-and-hot-path-perf-baselines.md)
- Standards: [Performance and complexity discipline](d:/RepoBrainOS/docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md)
- Logs / Artifacts: `cargo xtask perf` output and repo-native validation commands above
