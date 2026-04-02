# Verification Record: Reverse Import Index For Interactive Graph Paths

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the graph hot-path performance correction covering:

- removal of repeated full-import rescans for reverse-neighbor lookup
- reverse-import index construction inside `SnapshotGraphStore`
- continued correctness of blast-radius and broker flows after the refactor
- a durable performance guardrail against repeated whole-edge rescans on interactive graph paths
- an automated `xtask policy` check that rejects direct `self.snapshot.imports` access in `repobrain-graph`

## Intent / Spec References

- Intent: [Reverse import index for interactive graph paths intent](d:/RepoBrainOS/plans/2026-03-30-reverse-import-index-for-interactive-graph-paths-intent.md)
- Spec: [Reverse import index for interactive graph paths spec](d:/RepoBrainOS/plans/2026-03-30-reverse-import-index-for-interactive-graph-paths-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-graph`
- `cargo test -p repobrain-broker`
- `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- get-brief --goal "understand RepositoryScanner before editing" --scope RepositoryScanner --token-budget 4096 --repo-root d:\RepoBrainOS`
- `rg -n "self\\.snapshot\\.imports" src/rust/crates/repobrain-graph/src/lib.rs`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-graph` no longer rescans the full import vector for each seed path when it needs reverse-neighbor lookup on interactive graph paths.

`SnapshotGraphStore::new` now builds a reverse-import index once, and the affected reverse-neighbor code paths use that maintained helper instead of repeating:

- whole-import scans in `impacted_paths`
- whole-import scans in interactive traversal import expansion
- whole-import scans in seed-local reference edge assembly

The regression check confirmed the old reverse-scan pattern no longer appears in `repobrain-graph/src/lib.rs`.

`cargo xtask policy` now also fails if `repobrain-graph` directly reaches for `self.snapshot.imports` in store methods, forcing graph code to use maintained forward or reverse lookup helpers instead of quietly reintroducing raw rescans.

Smoke validation on the current repo still produced stable user-visible behavior:

- `blast-radius RepositoryScanner` returned 6 impacted nodes and 5 relationships
- `get-brief` still returned exact symbol guidance, structural adjacency, and deterministic verification targets for the same scope

## Pass / Fail Against Expectations

Pass.

This slice removes an avoidable `O(m * n)` reverse-neighbor path from an interactive graph surface without changing the product behavior built on top of it.

## Performance / Complexity Validation

- Measured:
  graph and broker tests passed; CLI smoke commands passed; full repo-native validation passed; the old reverse-scan pattern no longer appears in the graph source; policy now enforces the guardrail automatically
- Inferred:
  reverse-neighbor lookup now pays one store-local reverse-index build plus keyed lookup instead of repeated whole-import rescans per seed path
- Why no benchmark was needed, if applicable:
  this was a correctness-of-asymptotic-shape fix for an obviously avoidable repeated scan in a hot path; the main requirement was to remove the weak algorithmic structure and preserve behavior

## Residual Risks

- the reverse-import index is store-local, not snapshot-persisted
- other future edge families could still regress if they skip indexed reverse lookup and no targeted guardrail is added for their raw vector access pattern
- this slice improves the graph hot path, but does not yet add a benchmark harness for graph latency

## Related

- Standards: [Performance and complexity discipline](d:/RepoBrainOS/docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md)
- SDD / ADR: [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md), [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
