# Verification Record: Symbol-Fact And Path-Owned Lookup Indexes

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the lookup-structure correction covering:

- index-backed symbol fact-id lookup in ingest
- index-backed path-owned symbol lookup in ingest
- removal of path-based global symbol scans from graph traversal and blast radius
- backward-compatible rebuild of derived lookup indexes for legacy snapshots
- targeted policy guardrails against reintroducing the fixed anti-patterns

## Intent / Spec References

- Intent: [Symbol-fact and path-owned lookup indexes intent](d:/RepoBrainOS/plans/2026-03-30-symbol-fact-and-path-owned-lookup-indexes-intent.md)
- Spec: [Symbol-fact and path-owned lookup indexes spec](d:/RepoBrainOS/plans/2026-03-30-symbol-fact-and-path-owned-lookup-indexes-spec.md)

## Commands / Checks Run

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

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-ingest` now rebuilds maintained symbol lookup indexes during snapshot normalization:

- a fact-id lookup index for exact symbol identity
- a path-owned symbol lookup index over a contiguous symbol-order span

`repobrain-graph` now consumes those lookup boundaries instead of filtering the full global symbol vector by `relative_path`.

Legacy snapshots without the new derived fields still load correctly because normalization rebuilds the missing indexes before the snapshot is used.

`cargo xtask policy` now also rejects:

- direct `self.snapshot.symbols.iter()` scans in `repobrain-graph`
- linear fact-id lookup over `snapshot.symbols` in `repobrain-ingest`

## Pass / Fail Against Expectations

Pass.

This slice removes avoidable global symbol scans from interactive lookup paths without changing product behavior.

## Performance / Complexity Validation

- Workload exercised:
  ingest, graph, broker, CLI, and full repo-native validation over the current workspace
- Measured:
  all listed tests and repo-native validation commands passed; scan, blast-radius, and get-brief still behaved correctly on the current repo
- Inferred:
  symbol fact lookup improved from `O(s)` scan to `O(log s)` indexed lookup, and path-owned symbol expansion improved from `O(log p + k)` per path instead of global-symbol filtering
- Why no benchmark was needed, if applicable:
  this was a correction of obviously avoidable global scans on an interactive path; the main requirement was to replace the weak lookup structure with maintained indexes and preserve behavior

## Residual Risks

- the new symbol lookup indexes add snapshot artifact size and resident state
- import reverse lookup is still store-local in graph rather than snapshot-persisted
- benchmark thresholds for graph and broker hot paths are still future work

## Related

- Standards: [Performance and complexity discipline](d:/RepoBrainOS/docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md)
- SDD / ADR: [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md), [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
