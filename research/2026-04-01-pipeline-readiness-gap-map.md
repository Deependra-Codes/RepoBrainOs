# Research: Pipeline Readiness Gap Map

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

Which parts of the RepoBrain pipeline are fully set up today, and which parts are still missing in the current deterministic slice?

## Candidate Options

1. Continue implementation without a status audit.
2. Keep readiness notes informal in chat and PR summaries.
3. Create one evidence-backed gap map before further implementation.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. No audit | Fastest short-term coding | No shared status baseline | High drift and wrong priorities | Low | Reject |
| 2. Informal notes | Low friction | Hard to track over time | Inconsistent truth source | Low | Reject |
| 3. Evidence-backed gap map | Clear shared baseline and ordering | Requires docs maintenance | Low if indexed and checked | Medium | Recommended |

## Sources

- Primary:
  - [CLI main](d:/RepoBrainOS/src/rust/crates/repobrain-cli/src/main.rs)
  - [Graph service](d:/RepoBrainOS/src/rust/crates/repobrain-graph/src/lib.rs)
  - [Broker](d:/RepoBrainOS/src/rust/crates/repobrain-broker/src/lib.rs)
  - [Ingest](d:/RepoBrainOS/src/rust/crates/repobrain-ingest/src/lib.rs)
  - [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md)
  - [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- Secondary:
  - none

## Current Pipeline Status

### Implemented

- Snapshot scan/store/load pipeline is working (`scan`, snapshot artifact store).
- Structural blast radius works for direct one-hop adjacency with deterministic checks.
- Deterministic invariants are now emitted in graph and broker `do_not_break`.
- `get-brief` returns coverage audit and verification planning.
- `explain-flow` returns structural flow capsules.
- `what-changed-semantically` now returns deterministic snapshot-to-snapshot structural deltas.
- semantic diff now supports an L1 Rust bounded stage with explicit bounds and timeout policy receipts.
- semantic diff now supports an L2 relational semantic stage with deterministic function alignment and bounded witness receipts.
- decision-style broker requests now extract deterministic decision evidence from snapshot-indexed markdown/ADR artifacts.
- graph relationship builder now emits additional edge families (`imports`, `calls` where inferable, `tests`, `builds`, `documents`, `supports_decision`) in blast radius reports.
- CLI now exposes overlay-aware and readiness-aware serving inputs (`overlay_kind`, `claim_scope`, `overlay_hash`, `touched_paths`, readiness assessment fields).
- CLI now exposes an incremental ingest entrypoint (`scan-delta`) for changed-path delta merges over stored snapshots.
- MCP command bridge maps tool names to CLI commands.

### Not Yet Set Up

- **Semantic equivalence hardening beyond first L1 slice**:
  L1 now runs targeted `#[kani::proof]` obligation execution with bounded per-obligation time slicing, and L2 now runs deeper relational alignment with behavioral-profile tiers; remaining backlog is solver-backed L2 obligations, richer counterexample minimization, and contract-oriented obligation synthesis.
- **Pipeline hardening beyond first full setup**:
  decision evidence ranking can still return broad documentation hits; relationship expansion can produce high edge volumes on large scopes; and `scan-delta` currently performs bounded merge over fresh full snapshots rather than parser-level partial re-extraction.

### Intentionally Deferred In This Slice

- automatic verification command execution
- semantic/LLM-based flow synthesis
- dense retrieval or second-pass widening planner

## Recommendation

Next hardening priorities:

1. Tighten decision evidence ranking toward scope-local ADR/spec artifacts.
2. Add blast-radius relationship budgets/configuration for large repositories.
3. Add contract-oriented harness synthesis and obligation triage for large changed scopes.
4. Add solver-backed obligations for bounded L2 pair checks where toolchains exist.

## Direct Evidence vs Inference

- Direct:
  - `what-changed-semantically` now emits structural delta statuses (`reference_snapshot_missing`, `no_structural_change`, or `implemented`).
  - semantic diff output now includes explicit equivalence contract/evidence fields and staged backlog markers.
  - L1 check mode now discovers scoped `#[kani::proof]` harnesses and runs obligation-targeted Kani commands.
  - L2 alignment now includes behavioral-profile exact matching before same-name/similarity fallback tiers.
  - snapshot file metadata now includes deterministic `content_hash`, so same-size file edits register as structural modifications.
  - decision-style `get-brief` now surfaces non-empty `relevant_decisions` and can satisfy `decision_evidence` coverage.
  - graph relationship construction now emits non-structural edge families in blast-radius output.
  - CLI accepts and propagates non-default readiness/overlay inputs.
  - `scan-delta` command now persists changed-path delta-merged snapshots.
- Inferred:
  - current pipeline is now feature-complete for deterministic v1 setup, with remaining work concentrated in precision and formal-hardening depth.

## Unknowns / Follow-Ups

- how to keep decision evidence high-precision without sacrificing deterministic breadth
- whether L2 should prefer fewer high-confidence pairs over wider pair coverage by default
- whether `scan-delta` should evolve toward ingest-layer selective extraction APIs

## Engineering Impact

- Contract / type impact:
  none for this docs-only slice
- Testing impact:
  command smoke checks used as evidence
- Runtime / latency impact:
  none for this docs-only slice
- Workload assumptions:
  developer-driven safe-edit and architecture queries remain primary
- Dominant operations and cost centers:
  currently exact + structural retrieval and broker packing
- Candidate data structures / indexes:
  existing snapshot indexes are sufficient for current deterministic stage
- Main rejected alternative and why:
  undocumented implementation-first expansion was rejected due to drift risk
- Time complexity / constant-factor impact:
  none
- Memory / allocation impact:
  none
- Measurement plan or reason no benchmark is needed:
  no benchmark needed for documentation-only gap map
