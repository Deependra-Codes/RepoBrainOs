# Intent: Symbol-Fact And Path-Owned Lookup Indexes

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain still had avoidable linear scans on interactive lookup paths:

- fact-id lookup over the global symbol vector
- path-owned symbol lookup by filtering the global symbol vector

Those choices are structurally wrong for repo-scale snapshots and violate the repo's workload-first performance discipline.

## Why Now

The graph and broker layers now depend on symbol fact IDs and path-owned symbol discovery often enough that weak lookup structure would keep compounding latency cost as the repo and product grow.

## Goals

- make symbol fact-id lookup indexed instead of linear
- make path-owned symbol lookup indexed instead of global-symbol filtering
- make graph code consume ingest-owned lookup boundaries instead of raw vector scans
- add targeted policy guardrails against reintroducing these anti-patterns

## Non-Goals

- persisting every possible reverse graph index in the snapshot artifact
- changing symbol extraction semantics
- adding a full benchmark harness in this slice

## Constraints

- Technical:
  - keep snapshot artifacts backward-compatible with older JSON that lacks the new derived indexes
- Product:
  - preserve current blast-radius, get-brief, and CLI behavior
- Time / Team:
  - choose the smallest clean index set that fixes the real lookup workload

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale symbol tables and repeated interactive graph / broker queries
- Hot, warm, or cold path:
  warm-to-hot interactive lookup path
- Dominant operations and expected frequency:
  exact fact-id lookup for stable symbol identity; repeated path-to-symbol expansion during graph traversal and blast-radius planning
- Latency / throughput / memory sensitivity:
  latency-sensitive interactive path; modest extra snapshot memory is acceptable to remove repeated global scans
- Acceptable simplicity-over-speed tradeoff, if any:
  none for the fact-id and path-owned lookup paths
- Likely failure mode at 10x scale:
  graph and broker surfaces keep paying repeated global-symbol scans as snapshot size grows

## Success Metrics

- `symbol_fact_lookup` stops scanning the full symbol vector
- path-owned symbol lookup becomes indexed and reusable
- graph blast-radius and define traversal stop scanning the full symbol table by path
- repo-native validation stays green

## Risks of Inaction

- symbol-heavy repos pay unnecessary interactive latency
- graph code keeps rebuilding access patterns over raw vectors instead of using owned lookup boundaries
- future contributors may copy the same weak lookup shape

## Research Scope

- workload-matched index design for exact fact lookup and path-owned symbol lookup
- backward-compatible derived-index rebuild inside snapshot normalization

## Acceptance Shape

- Primary user-visible outcomes:
  same product behavior with lower structural lookup cost
- Invariants that must remain true:
  older snapshots still load and rebuild the missing indexes
- Verification targets:
  `cargo test -p repobrain-ingest`
  `cargo test -p repobrain-graph`
  `cargo test -p repobrain-broker`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
