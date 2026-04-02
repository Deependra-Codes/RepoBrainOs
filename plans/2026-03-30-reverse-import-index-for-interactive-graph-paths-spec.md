# Spec: Reverse Import Index For Interactive Graph Paths

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Reverse import index for interactive graph paths intent](d:/RepoBrainOS/plans/2026-03-30-reverse-import-index-for-interactive-graph-paths-intent.md)

## Problem Statement

Interactive graph queries currently pay repeated full-import scan cost for reverse-neighbor lookup.

That is an avoidable bad asymptotic path under RepoBrain’s own performance discipline.

## Scope

- add a reverse-import index inside `SnapshotGraphStore`
- route reverse-neighbor lookups through an explicit helper
- remove direct full-import rescans from the affected interactive graph paths
- add a written guardrail in the performance standard

## Non-Goals

- changing snapshot artifact format
- generic indexing for every future edge type
- graph persistence redesign

## Behavioral Requirements

1. `SnapshotGraphStore::new` must build a reverse-import index from resolved path to importer edges.
2. `impacted_paths`, `traverse`, and seed-local reference assembly must use the indexed reverse lookup instead of rescanning all imports for every seed path.
3. Reverse lookup behavior must stay equivalent to the prior implementation.
4. The performance standard must explicitly forbid repeated whole-edge rescans on interactive graph paths when a maintained reverse index is practical.

## Acceptance Examples

1. A blast-radius query for a symbol with reverse importers still includes the importing file.
2. A path-target blast-radius query still finds reverse importers without scanning the entire import set per seed.
3. The graph file no longer contains the old `for import in &self.snapshot.imports` reverse-neighbor loops in the affected hot paths.

## Invariants

- Must always hold:
  - reverse importer lookup stays snapshot-bound
  - reverse index contents must only include imports with a resolved path
- Must not regress:
  - current one-hop blast-radius behavior
  - current CLI graph demos
  - current broker guidance built on graph reports

## Contract And Type Changes

- Schema changes:
  - none
- Public interface changes:
  - none outside graph internals and documentation
- Illegal states to remove:
  - repeated full-import rescans for reverse-neighbor lookup on interactive graph paths

## Workload And Complexity Notes

- Workload shape and expected scale:
  - repeated one-hop graph queries over maintained snapshots with small seed sets and potentially much larger import sets
- Hot, warm, or cold path:
  - hot interactive path
- Chosen data structures and why:
  - `BTreeMap<&str, Vec<&IndexedImport>>` reverse index in `SnapshotGraphStore`
  - chosen because it matches the existing deterministic, ordered style and provides bounded keyed lookup without changing snapshot storage
- Key operation costs:
  - reverse index build is `O(n log k)` over imports when the store is created
  - reverse-neighbor lookup becomes `O(log k + r)` instead of rescanning all `n` imports for each seed path
- Memory / allocation notes:
  - store-local reverse index adds modest temporary memory proportional to indexed imports
- Measured vs inferred performance claims:
  - inferred complexity improvement from removing repeated full-import scans on interactive graph paths

## Test Strategy

- Unit:
  - reverse-neighbor graph tests continue to pass
- Integration:
  - CLI blast-radius and get-brief smoke checks
- Regression:
  - graph code path still returns reverse importer results after the refactor
- Property / invariant:
  - reverse index includes only imports with resolved paths

## Verification Notes

- Commands:
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated validation record for this slice
