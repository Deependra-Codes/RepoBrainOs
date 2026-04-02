# Intent: Reverse Import Index For Interactive Graph Paths

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

`repobrain-graph` currently rescans the full import set when it needs reverse neighbors for a seed path.

That creates avoidable `O(m * n)` behavior in interactive graph paths where:

- `m` is the number of seed paths
- `n` is the number of indexed imports

The current behavior is correct, but the asymptotic shape is wrong for a repo-scale interactive path.

## Why Now

The graph layer now sits directly on the critical path for:

- `blast-radius`
- `get-brief`
- structural flow and evidence assembly

This is exactly where the repo’s performance discipline says avoidable repeated full rescans should be removed.

## Goals

- remove repeated whole-import rescans from interactive graph paths
- add an explicit reverse-import index boundary inside `SnapshotGraphStore`
- preserve current graph behavior while improving cost shape
- leave a durable guardrail so reverse graph work uses the indexed path by default

## Non-Goals

- storage engine redesign
- multi-hop graph planning
- symbol or call indexes in this slice
- benchmark harness work in this slice

## Constraints

- Technical:
  - must preserve current blast-radius and traversal behavior
  - must keep the fix local to graph-layer query structure, not widen snapshot contracts unnecessarily
- Product:
  - no user-facing regression in blast-radius or briefing output
- Time / Team:
  - bounded refactor, not a generalized graph engine rebuild

## Workload And Performance Shape

- Expected input size / scale path:
  - one-hop interactive graph work over repo snapshots with potentially hundreds of imports and repeated reverse-neighbor lookups
- Hot, warm, or cold path:
  - hot interactive path
- Latency / throughput / memory sensitivity:
  - repeated reverse-neighbor lookup should not rescan the full edge set for each seed path
- Acceptable simplicity-over-speed tradeoff, if any:
  - a small upfront per-store reverse index is acceptable because it avoids repeated interactive rescans

## Success Metrics

- reverse-neighbor graph work no longer rescans `self.snapshot.imports` per seed path
- blast-radius and traversal behavior remain stable
- the repo has a written guardrail against repeated whole-edge rescans on interactive graph paths

## Risks of Inaction

- the graph path keeps avoidable repo-scale slowdowns
- future graph work may copy the same rescan pattern into new edge families

## Research Scope

- [Performance and complexity discipline](d:/RepoBrainOS/research/2026-03-30-performance-and-complexity-discipline.md)
- current graph implementation and hot-path loops

## Acceptance Shape

- Primary user-visible outcomes:
  - `blast-radius` and `get-brief` still work with unchanged or cleaner output
- Invariants that must remain true:
  - reverse importer discovery remains correct
  - graph results remain snapshot-bound and one-hop
- Verification targets:
  - `cargo test -p repobrain-graph`
  - `cargo test -p repobrain-broker`
  - `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\\RepoBrainOS`
  - `cargo run -p repobrain-cli -- get-brief --goal \"understand RepositoryScanner before editing\" --scope RepositoryScanner --token-budget 4096 --repo-root d:\\RepoBrainOS`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
