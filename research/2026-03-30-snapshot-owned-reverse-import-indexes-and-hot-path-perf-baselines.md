# Research: Snapshot-Owned Reverse Import Indexes And Hot-Path Perf Baselines

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Research Question

What is the cleanest next step to remove the remaining reverse-import ownership leak and add honest, repeatable latency baselines for RepoBrain's hot graph and broker paths?

## Candidate Options

1. Keep reverse import lookup store-local in `repobrain-graph` and keep perf validation as prose-only.
2. Persist snapshot-owned reverse import indexes, but delay explicit perf measurement.
3. Persist snapshot-owned reverse import indexes and add a dedicated repo-native perf command with stable synthetic workload thresholds.
4. Add heavy benchmark infrastructure or integrate perf thresholds into every default validation command immediately.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Keep store-local reverse lookup | Lowest implementation cost | Ownership stays wrong | Graph keeps rebuilding maintained state | Low | Reject |
| 2. Snapshot-owned reverse lookup only | Fixes boundary ownership | Hot-path perf still stays unmeasured | Residual latency claims remain inference-only | Medium | Better, but incomplete |
| 3. Snapshot-owned reverse lookup plus dedicated perf command | Fixes ownership and creates repeatable measurement | Adds benchmark fixture and thresholds to maintain | Must keep thresholds realistic and non-flaky | Medium | Recommended |
| 4. Heavy benchmark infra everywhere | Highest rigor in theory | High maintenance and higher flake risk | Benchmark theater and slow feedback loops | High | Reject |

## Main Conclusion

RepoBrain should finish this boundary properly:

- reverse import adjacency belongs to the maintained snapshot, not the graph store
- hot graph and broker paths should have an explicit repo-native perf surface with thresholds

The right tool is not "run performance on every command forever."

The right tool is:

- keep the owned lookup structures in ingest
- make the graph a pure consumer of maintained indexes
- add a synthetic, repeatable perf fixture that measures `blast-radius` and `get-brief`
- enforce thresholds through a dedicated `cargo xtask perf` command, not through every default check

## What The Existing Repo Research Suggests

### 1. Latency-first architecture wants maintained read structures

[Latency-first strategy](d:/RepoBrainOS/research/2026-03-30-latency-first-strategy.md) already argues that the hot path must read from maintained state rather than regenerate expensive structure on demand.

Implication:

- reverse import adjacency should be part of the maintained snapshot layer
- graph should not rebuild that adjacency every time it is constructed

### 2. Performance discipline should avoid repeated cost and benchmark theater at the same time

[Performance release gate and workload-first data-structure enforcement](d:/RepoBrainOS/research/2026-03-30-performance-release-gate-and-workload-first-data-structure-enforcement.md) already established two important rules:

- weak structure choices should not ship on scale-sensitive paths
- not every change needs full benchmark ceremony

Implication:

- a dedicated perf command with thresholds fits the repo better than always-on benchmark gates
- the measurement method should be explicit and repeatable

### 3. The roadmap already called for benchmark-first CLI and hot-path measurement

[Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md) explicitly recommends benchmarkable surfaces for `blast-radius`, `get-brief`, and future `explain-flow`.

Implication:

- this slice is directly on the roadmap, not an extra side quest

## Design Conclusions For RepoBrain

### 1. Persist reverse import lookup as a derived snapshot index

Use the same pattern as the new symbol indexes:

- keep `imports` as the canonical fact vector
- build a deterministic reverse-import order over import indexes sorted by `resolved_path`
- store path spans as derived snapshot metadata

### 2. Use a synthetic perf fixture instead of the live repo for thresholds

The live repo changes too often for stable thresholds.

A synthetic Rust fixture with many reverse importers around one anchor symbol gives:

- repeatable shape
- meaningful reverse-neighbor pressure
- stable graph and broker exercise

### 3. Gate perf through an explicit repo-native command

`cargo xtask perf` should:

- build the synthetic workload
- scan it once
- warm up the hot paths
- measure repeated `blast-radius` and `get-brief`
- enforce generous but real p95 thresholds

### 4. Keep default validation separate from perf thresholds

`cargo xtask quality` and `cargo xtask check` should stay stable and low-chaos.

Hot-path perf should still be first-class, but not forced into every single default validation run.

## Recommendation

Adopt Option 3:

- snapshot-owned reverse import indexes in ingest
- graph consumes only maintained forward and reverse lookup boundaries
- `cargo xtask perf` enforces p95 baselines for `blast-radius` and `get-brief`

## Direct Evidence vs Inference

- Direct:
  - repo-local latency and performance research already says maintained read structures are preferred on hot paths and measurement discipline matters
  - the current residual risk explicitly names reverse-import ownership and missing perf thresholds as the remaining gaps
- Inferred:
  - a dedicated perf command with synthetic thresholds is the lowest-chaos way to add real measurement without turning every repo command into a benchmark harness

## Unknowns / Follow-Ups

- whether future `explain-flow` should join the same perf surface
- whether thresholds should eventually be split by debug vs release profile
- whether more graph edge families will also need snapshot-owned reverse indexes

## Engineering Impact

- Contract / type impact:
  - add derived reverse-import index fields to `RepositoryInventorySnapshot`
- Testing impact:
  - extend ingest tests for reverse lookup and legacy snapshot backfill
  - validate graph and broker behavior still works on top of the new index
- Runtime / latency impact:
  - reverse import adjacency becomes snapshot-owned
  - graph construction stops rebuilding that adjacency
  - hot-path latency claims gain measured baselines
- Workload assumptions:
  - repeated interactive graph and broker queries against warm snapshot state
- Dominant operations and cost centers:
  - reverse-neighbor expansion in graph
  - `blast-radius` report assembly
  - `get-brief` broker compilation
- Candidate data structures / indexes:
  - deterministic reverse-import order plus path-span index
  - synthetic perf fixture plus sorted duration samples
- Main rejected alternative and why:
  - live-repo thresholds were rejected because repo contents drift too often for stable perf budgets
- Time complexity / constant-factor impact:
  - reverse import lookup becomes `O(log r + k)` over maintained spans rather than graph-local rebuild cost plus lookup
  - perf command adds explicit measurement cost only when invoked
- Memory / allocation impact:
  - snapshot stores extra reverse-import derived vectors
  - graph store drops its local reverse-import map
- Measurement plan or reason no benchmark is needed:
  - this slice does add a benchmark-like repo-native perf command because the remaining risk is specifically about unmeasured hot-path thresholds

## Sources

- Primary:
  - [Latency-first strategy](d:/RepoBrainOS/research/2026-03-30-latency-first-strategy.md)
  - [Performance release gate and workload-first data-structure enforcement](d:/RepoBrainOS/research/2026-03-30-performance-release-gate-and-workload-first-data-structure-enforcement.md)
  - [Next-level roadmap for ingest, graph, broker, and CLI](d:/RepoBrainOS/research/2026-03-30-next-level-roadmap-for-ingest-graph-broker-cli.md)
- Secondary:
  - none
