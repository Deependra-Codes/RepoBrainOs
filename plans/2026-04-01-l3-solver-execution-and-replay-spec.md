# Spec: L3 Solver Execution And Replay

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [L3 solver execution and replay](d:/RepoBrainOS/plans/2026-04-01-l3-solver-execution-and-replay-intent.md)

## Problem Statement

L3 theorem output previously generated obligations but returned placeholder inconclusive certificates with no replay artifact path.

## Scope

- execute L3 obligations through a bounded deterministic relational solver lane
- emit per-obligation theorem certificates with explicit status and solver metadata
- persist replay artifacts under `.repobrain/theorem-runs/` or explicit output path
- add `replay-theorem` CLI command for artifact inspection
- add optional Alive2 sidecar evidence probe
- add deterministic scheduling and fairness controls

## Non-Goals

- full SMT/IVL obligation proving across arbitrary language/runtime semantics
- Alive2 per-obligation LLVM IR validation in this slice
- cross-machine distributed theorem execution

## Behavioral Requirements

1. L3 must emit obligation-level certificate outcomes (`proved`, `refuted`, `inconclusive`).
2. Replay artifacts must persist solver metadata, witnesses, and encoding hashes.
3. Replay command must load and render replay artifact records deterministically.
4. Scheduling must be deterministic and stable across repeated runs.
5. Timeout policy must be bounded and fair at per-obligation level.
6. Translation-validation sidecar must be optional and must never over-claim proof when unavailable.

## Acceptance Examples

1. With aligned unchanged relational signatures, L3 emits `proved_under_contract`.
2. With unmatched obligations, L3 emits `refuted_under_contract` with obligation witness.
3. With missing solver sidecar, translation lane emits explicit `inconclusive` evidence.
4. Replay artifact can be loaded by `repobrain replay-theorem --artifact <path>`.

## Invariants

- Must always hold:
  L3 claims remain bounded by contract and configured policy.
- Must not regress:
  deterministic ordering for obligations/certificates/replay records.

## Contract And Type Changes

- Domain:
  `TheoremProofCertificate` now includes optional `encoding_hash`.
- Schemas:
  added `theorem-replay-artifact.schema.json`
  updated `theorem-proof-certificate.schema.json` with `encoding_hash`
- CLI output:
  added `theorem_replay_artifact` and `theorem_replay_command`

## Workload And Complexity Notes

- Workload shape:
  bounded obligation scheduling + bounded per-obligation execution + optional sidecar probe
- Path class:
  warm/cold verification path
- Dominant operations:
  deterministic map/set scheduling, per-obligation evaluation, artifact serialization
- Data structures:
  `BTreeMap` / `BTreeSet` for deterministic ordering

## Test Strategy

- Unit:
  scheduling priority/determinism, theorem execution status mapping, translation sidecar behavior
- Integration:
  theorem replay artifact persistence and replay command output
- Regression:
  repeated theorem runs produce stable obligation/certificate ordering

## Verification Notes

- Commands:
  `cargo test -p repobrain-cli`
  `cargo test -p repobrain-domain`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
- Artifacts:
  updated CLI/domain/schema surfaces and L3 theorem runtime behavior
