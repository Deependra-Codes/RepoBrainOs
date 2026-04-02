# Spec: Symbol-Fact And Path-Owned Lookup Indexes

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Symbol-fact and path-owned lookup indexes intent](d:/RepoBrainOS/plans/2026-03-30-symbol-fact-and-path-owned-lookup-indexes-intent.md)

## Problem Statement

The current snapshot shape supported exact symbol-name lookup efficiently, but it did not support:

- exact symbol lookup by stable `fact_id`
- path-owned symbol lookup without scanning the full symbol vector

That left interactive graph and broker paths paying avoidable global-scan costs.

## Scope

- add maintained derived indexes for symbol fact-id lookup and path-owned symbol lookup inside `RepositoryInventorySnapshot`
- rebuild those indexes during snapshot normalization for backward compatibility
- update graph code to consume the new lookup boundaries
- add targeted policy guardrails for the fixed anti-patterns

## Non-Goals

- changing runtime contracts outside the snapshot artifact
- persisting additional graph-specific reverse indexes beyond what is needed here
- changing semantic extraction quality

## Behavioral Requirements

1. `symbol_fact_lookup` must resolve via an indexed lookup path rather than a linear scan.
2. `RepositoryInventorySnapshot` must expose a path-owned symbol lookup boundary for graph use.
3. Legacy snapshots without the derived indexes must rebuild them during normalization.
4. Graph code must stop filtering the full symbol vector by `relative_path`.
5. `cargo xtask policy` must fail if the fixed anti-patterns are reintroduced in ingest or graph.

## Acceptance Examples

1. Given a scanned snapshot, looking up `RepositoryScanner` by `fact_id` returns the symbol without scanning `symbols`.
2. Given a path with multiple symbols, the graph can enumerate them through the path-owned lookup boundary.
3. Given a legacy snapshot JSON with symbols but no new index fields, load still succeeds and the lookup methods work.

## Invariants

- Must always hold:
  - exact symbol-name lookup stays `O(log n + k)` over the sorted name slice
  - exact symbol fact-id lookup is index-backed
  - path-owned symbol lookup is index-backed
- Must not regress:
  - snapshot serialization and legacy deserialization remain valid
  - graph blast-radius and traversal behavior stay stable

## Contract And Type Changes

- Schema changes:
  - none to cross-language schemas
- Public interface changes:
  - `RepositoryInventorySnapshot` adds path-owned and fact-owned symbol lookup methods
- Illegal states to remove:
  - snapshots with symbols but no rebuilt derived lookup indexes after normalization

## Workload And Complexity Notes

- Workload shape and expected scale:
  repeated interactive graph and broker queries over repo-scale symbol tables
- Hot, warm, or cold path:
  warm-to-hot interactive path
- Dominant operations and expected frequency:
  stable fact lookup, path-to-symbol expansion, define traversal
- Chosen data structures and why:
  sorted derived vectors for `fact_id -> symbol_index` and `path -> contiguous symbol-index span`; they preserve deterministic artifacts and support binary-search-backed lookup without adding hash-map nondeterminism to the stored snapshot
- Main alternative considered:
  a `BTreeMap` stored in-memory only was rejected because the snapshot already has a normalize step and benefits from persisted, deterministic derived indexes plus legacy backfill
- Index ownership / maintenance notes:
  `RepositoryInventorySnapshot::normalize` owns rebuilding the derived symbol lookup indexes
- Key operation costs:
  fact lookup is `O(log s)`; path-owned symbol lookup is `O(log p + k)` where `p` is indexed paths and `k` is symbols on the matched path
- Memory / allocation notes:
  snapshot artifacts now store extra derived vectors for symbol lookup; this modest resident-state increase is accepted to remove repeated global scans on interactive paths
- Measured vs inferred performance claims:
  measured: behavior and validation stay green; inferred: interactive symbol lookup shape improves from repeated `O(s)` scans to binary-search-backed lookup
- Benchmark expectation or reason none is needed:
  no benchmark was needed for this slice because the main requirement was removing obviously avoidable global scans and replacing them with maintained indexed lookup structure

## Test Strategy

- Unit:
  add lookup behavior tests in ingest
- Integration:
  validate blast-radius and get-brief still work through graph and broker tests
- Regression:
  legacy snapshot load must rebuild the new lookup indexes
- Property / invariant:
  path-owned symbol lookup returns only symbols for the requested path

## Verification Notes

- Commands:
  - `cargo test -p repobrain-ingest`
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
  - `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal "understand RepositoryScanner before editing" --scope RepositoryScanner --token-budget 4096 --repo-root d:\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated ingest lookup boundaries, graph traversal logic, and policy guardrails
