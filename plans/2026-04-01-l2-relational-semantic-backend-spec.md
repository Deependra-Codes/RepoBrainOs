# Spec: L2 Relational Semantic Backend

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [L2 relational semantic backend intent](d:/RepoBrainOS/plans/2026-04-01-l2-relational-semantic-backend-intent.md)

## Problem Statement

RepoBrain needs an executable L2 semantic-diff stage that performs deterministic function alignment and bounded relational comparison, while preserving explicit non-proof semantics.

## Scope

- add L2 stage selection to semantic diff CLI
- add L2 policy args:
  - `--l2-alignment-mode`
  - `--l2-max-pairs`
  - `--l2-max-candidate-functions`
  - `--l2-timeout-ms`
  - `--l2-min-alignment-score`
- build changed-path function signals from both reference and target snapshots
- run deterministic alignment tiers:
  - exact path+name matching
  - same-name language matching
  - optional signature-aware similarity matching
- run bounded relational comparison across aligned pairs and unmatched residues
- emit L2 evidence receipts with bounds, timeout, assumptions, and witness

## Non-Goals

- full semantic equivalence theorem proving
- cross-process external solver orchestration as a hard dependency for L2 baseline
- inference that no observed difference implies unrestricted equivalence

## Behavioral Requirements

1. L2 mode must be opt-in via `--stage l2-relational-semantic`.
2. L2 receipts must include explicit bounds and timeout values.
3. L2 output must stay machine-readable and deterministic.
4. Missing reference snapshot behavior remains deterministic and stage-aware.
5. L2 wording must remain explicit that evidence is bounded and non-proof.

## Acceptance Examples

1. Running L2 over changed snapshots emits `equivalence_stage: l2_relational_semantic` and a status in `{l2_relational_no_counterexample, l2_relational_counterexample, l2_relational_inconclusive}`.
2. Missing reference with L2 selected emits `reference_snapshot_missing` with backend `l2_relational_semantic`.
3. Signature-aware mode can align renamed function symbols when other contextual signals remain consistent.

## Invariants

- Must always hold:
  alignment and receipt output are deterministic under the same snapshots and policy.
- Must not regress:
  L2 output must not claim unrestricted semantic equivalence proof.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  semantic diff CLI adds L2 policy flags and L2 status labels
- Illegal states to remove:
  L2 stage listed as planned-only with no executable backend

## Workload And Complexity Notes

- Workload shape and expected scale:
  bounded candidate construction + bounded pair scoring
- Hot, warm, or cold path:
  warm interactive path (opt-in)
- Dominant operations and expected frequency:
  `BTreeMap` / `BTreeSet` grouping and bounded similarity scoring
- Chosen data structures and why:
  ordered sets/maps preserve deterministic behavior and stable output ordering
- Key operation costs:
  alignment candidate pass is bounded by configured candidate and pair caps
- Measured vs inferred performance claims:
  complexity is inferred from bounded deterministic map/set and pair-scoring operations

## Test Strategy

- Unit:
  L2 status-label mapping test
  renamed-function alignment no-difference test under signature-aware policy
  import-context difference detection test
- Integration:
  L2 CLI smoke command with existing snapshots
- Regression:
  missing reference stage-aware L2 receipt behavior

## Verification Notes

- Commands:
  `cargo test -p repobrain-cli`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref DOES_NOT_EXIST --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
