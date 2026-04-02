# Research: Semantic Diff Pipeline

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

What is the best deterministic first implementation for `what-changed-semantically` without over-claiming semantic equivalence?

## Candidate Options

1. Keep `not_implemented` placeholder.
2. Return only changed file counts from snapshot file lists.
3. Return deterministic structural delta from files, symbol/import fact IDs, and verification targets.
4. Introduce AST-level semantic differencing immediately.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Keep placeholder | No implementation work | No user value | Pipeline gap persists | Low | Reject |
| 2. File-only diff | Small and fast | Misses fact-level signal | Weak semantic proxy | Low | Incomplete |
| 3. Structural multi-signal diff | Deterministic + explainable + bounded | More implementation than file-only | Still not true semantic equivalence | Medium | Recommended |
| 4. Full semantic diff now | Higher long-term ceiling | Heavy complexity and risk | Overbuild for current stage | High | Reject for now |

## Sources

- Primary:
  - [CLI semantic diff stub](d:/RepoBrainOS/src/rust/crates/repobrain-cli/src/main.rs)
  - [Ingest snapshot artifact store](d:/RepoBrainOS/src/rust/crates/repobrain-ingest/src/lib.rs)
  - [SDD-002 semantic delta propagation](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md)
  - [SDD-005 change detection and incremental update pipeline](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
  - [Pipeline readiness gap map](d:/RepoBrainOS/research/2026-04-01-pipeline-readiness-gap-map.md)
- Secondary:
  - none

## Recommendation

Implement Option 3 as the first semantic-diff slice:

- snapshot-vs-snapshot deterministic structural deltas
- changed scope derived from file and fact-level differences
- explicit wording that output is structural delta, not semantic equivalence

## Direct Evidence vs Inference

- Direct:
  - CLI currently emits `status: not_implemented` for semantic diff.
  - snapshot store already supports revision-labeled artifacts.
  - architecture docs position semantic delta as scoped impact estimation.
- Inferred:
  - a multi-signal structural delta is the strongest low-risk baseline before true semantic diff work.

## Unknowns / Follow-Ups

- when to add graph propagation for indirectly stale concepts
- whether semantic diff output should be promoted into broker-level API after CLI hardening
- which confidence scheme best communicates structural-vs-semantic certainty

## Engineering Impact

- Contract / type impact:
  none across shared schemas for this slice
- Testing impact:
  add CLI unit tests for snapshot delta helper behavior
- Runtime / latency impact:
  bounded linear scans over two snapshots
- Workload assumptions:
  user runs diffs against one reference snapshot and current/worktree snapshot
- Dominant operations and cost centers:
  set/map difference over files, symbols, imports, and verification targets
- Candidate data structures / indexes:
  transient `BTreeMap`/`BTreeSet` for deterministic output
- Main rejected alternative and why:
  file-only diff rejected because it misses fact-level changes that already exist in snapshots
- Time complexity / constant-factor impact:
  `O(n log n)` due deterministic ordered structures
- Memory / allocation impact:
  temporary maps/sets proportional to snapshot entities
- Measurement plan or reason no benchmark is needed:
  no dedicated benchmark in this first deterministic CLI slice
