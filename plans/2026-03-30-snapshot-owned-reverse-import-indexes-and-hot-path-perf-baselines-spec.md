# Spec: Snapshot-Owned Reverse Import Indexes And Hot-Path Perf Baselines

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Snapshot-owned reverse import indexes and hot-path perf baselines intent](d:/RepoBrainOS/plans/2026-03-30-snapshot-owned-reverse-import-indexes-and-hot-path-perf-baselines-intent.md)

## Problem Statement

The current system still had one ownership leak and one measurement gap:

- reverse import adjacency was rebuilt inside `repobrain-graph`
- `blast-radius` and `get-brief` had no measured threshold command

## Scope

- persist reverse import lookup as derived snapshot metadata in ingest
- update graph to consume snapshot-owned reverse lookup
- add `cargo xtask perf`
- enforce explicit p95 thresholds for synthetic `blast-radius` and `get-brief` workloads

## Non-Goals

- adding a new benchmark crate
- putting perf thresholds into every default validation command
- measuring unsupported or future commands in this slice

## Behavioral Requirements

1. `RepositoryInventorySnapshot` must expose reverse import lookup without requiring graph-local rebuild.
2. Legacy snapshots without the derived reverse import index must rebuild it during normalization.
3. `repobrain-graph` must stop constructing its own reverse import map.
4. `cargo xtask perf` must build a stable synthetic workload, measure repeated `blast-radius` and `get-brief`, and fail if the p95 threshold is exceeded.
5. `cargo xtask perf` must print the measured p50, p95, mean, and max for each measured path.

## Acceptance Examples

1. A reverse importer query for `src/shared.rs` on the synthetic fixture returns many importer files through the snapshot-owned reverse lookup.
2. `cargo xtask perf` succeeds when both hot paths stay within budget.
3. `cargo xtask perf` fails with a clear threshold error when a measured p95 exceeds the budget.

## Invariants

- Must always hold:
  - reverse import lookup is maintained by the snapshot layer
  - graph is a consumer of maintained reverse lookup
  - perf thresholds are measured on a stable synthetic workload
- Must not regress:
  - `blast-radius` and `get-brief` keep their current product behavior
  - default validation commands remain separate from perf thresholds

## Contract And Type Changes

- Schema changes:
  - none to cross-language schemas
- Public interface changes:
  - ingest adds snapshot-owned reverse import lookup
  - xtask adds `perf`
- Illegal states to remove:
  - graph-local reverse import adjacency as the only reverse lookup source

## Workload And Complexity Notes

- Workload shape and expected scale:
  repeated reverse-neighbor and briefing requests over warm snapshot state
- Hot, warm, or cold path:
  hot interactive graph and broker path; warm perf command for explicit measurement
- Dominant operations and expected frequency:
  reverse import expansion, relationship assembly, evidence-backed briefing assembly
- Chosen data structures and why:
  reverse-import order plus path-span index in ingest for deterministic snapshot ownership; sorted duration samples in `xtask perf` for stable percentile reporting
- Main alternative considered:
  graph-store-local reverse maps plus prose-only perf notes were rejected because they keep the ownership leak and leave thresholds unmeasured
- Index ownership / maintenance notes:
  `RepositoryInventorySnapshot::normalize` owns rebuilding reverse import indexes; `xtask perf` owns hot-path latency baselines
- Key operation costs:
  reverse import lookup is `O(log r + k)` over the maintained reverse span index; perf measurement is linear in the chosen iteration count
- Memory / allocation notes:
  snapshot stores extra reverse-import derived vectors; graph drops its local reverse map; perf command allocates one temporary synthetic fixture and duration samples
- Measured vs inferred performance claims:
  measured: `xtask perf` reports p50/p95/mean/max for the synthetic workload; inferred: snapshot-owned reverse lookup removes repeated graph setup cost
- Benchmark expectation or reason none is needed:
  this slice does include an explicit perf command because the remaining risk was the absence of measured hot-path thresholds

## Test Strategy

- Unit:
  ingest reverse lookup tests and legacy backfill tests
- Integration:
  graph and broker tests remain green on top of the new snapshot-owned reverse lookup
- Regression:
  `cargo xtask perf` provides the explicit hot-path regression surface
- Property / invariant:
  reverse import lookup returns only imports whose `resolved_path` matches the requested path

## Verification Notes

- Commands:
  - `cargo test -p repobrain-ingest`
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo xtask perf`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated ingest reverse index ownership, graph lookup logic, xtask perf command, and validation record
