# Spec: Semantic Diff Pipeline

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [Semantic diff pipeline intent](d:/RepoBrainOS/plans/2026-04-01-semantic-diff-pipeline-intent.md)

## Problem Statement

The semantic-diff CLI entrypoint exists but is stubbed. We need a deterministic first implementation that compares maintained snapshots and reports structural deltas.

## Scope

- implement `what-changed-semantically` in `repobrain-cli`
- load target snapshot via `--revision` or default `worktree`
- load reference snapshot via `--ref`
- compute deterministic deltas for:
  - file inventory (added/removed/metadata-modified paths)
  - symbol facts (added/removed counts + touched paths)
  - import facts (added/removed counts + touched paths)
  - verification targets (added/removed rendered checks)
- emit an explicit missing-reference status when reference snapshot artifact is absent

## Non-Goals

- semantic equivalence proofs
- intra-file AST differencing
- graph propagation for stale concept regeneration
- automatic reference snapshot generation

## Behavioral Requirements

1. The command must not return `not_implemented` once this slice lands.
2. The command must compare two snapshot artifacts deterministically.
3. Changed scope output must be deduplicated and deterministic.
4. If the reference snapshot artifact does not exist, the output must include a recovery step (`scan --revision <ref>`).
5. Summary text must explicitly label output as structural delta rather than semantic certainty.

## Acceptance Examples

1. If target and reference snapshots are equal, output reports no structural delta and an empty changed scope.
2. If a new file and symbol are introduced between snapshots, changed scope includes that path and symbol delta counts are non-zero.
3. If reference snapshot is missing, output status reflects missing reference snapshot without crashing.

## Invariants

- Must always hold:
  diff output is snapshot-bound and deterministic.
- Must not regress:
  semantic diff wording must not imply exact semantic equivalence checking.

## Contract And Type Changes

- Schema changes:
  none (CLI-local output struct only)
- Public interface changes:
  semantic diff CLI output gains deterministic delta fields
- Illegal states to remove:
  permanently stubbed semantic diff status

## Workload And Complexity Notes

- Workload shape and expected scale:
  linear set-diff over two snapshot vectors
- Hot, warm, or cold path:
  interactive CLI path
- Dominant operations and expected frequency:
  map/set construction and difference computation for file paths and fact IDs
- Chosen data structures and why:
  `BTreeMap` and `BTreeSet` for deterministic ordering and deduplication
- Main alternative considered:
  hash-only summary counters were rejected due weaker explainability and changed-scope support
- Index ownership / maintenance notes:
  no new persisted indexes; consume existing snapshot artifacts
- Key operation costs:
  `O(n log n)` set/map operations over snapshot entities
- Memory / allocation notes:
  temporary maps/sets proportional to snapshot size
- Measured vs inferred performance claims:
  inferred complexity from deterministic map/set operations
- Benchmark expectation or reason none is needed:
  no dedicated benchmark for this first deterministic CLI slice

## Test Strategy

- Unit:
  semantic delta helper tests for changed and unchanged snapshots
- Integration:
  CLI smoke command over repo-local snapshots
- Regression:
  missing reference snapshot status behavior
- Property / invariant:
  changed-scope ordering and deduplication remain deterministic

## Verification Notes

- Commands:
  `cargo test -p repobrain-cli`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
- Artifacts:
  updated CLI implementation/tests and a validation record
