# Spec: L1 Rust Bounded Backend (Kani)

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [L1 Rust bounded backend intent](d:/RepoBrainOS/plans/2026-04-01-l1-rust-bounded-kani-backend-intent.md)

## Problem Statement

RepoBrain needs a concrete L1 stage that runs bounded Rust formal checks with explicit policy controls and timeout governance.

## Scope

- add L1 stage selection to semantic diff CLI
- add policy args:
  - `--l1-max-rust-files`
  - `--l1-max-functions`
  - `--l1-timeout-ms`
  - `--l1-kani-mode` (`probe` | `check`)
- build deterministic Rust candidate scope from structural delta
- execute `cargo kani` with host-side timeout enforcement
- emit bounded evidence receipts for completed, timeout, and inconclusive outcomes

## Non-Goals

- proof harness synthesis for every changed function
- complete semantic equivalence guarantees
- cross-language bounded formal checking in this slice

## Behavioral Requirements

1. L0 remains default behavior.
2. L1 mode must be opt-in via `--stage l1-rust-bounded`.
3. L1 receipts must include bounds and timeout values.
4. Toolchain-unavailable or timeout outcomes must return `inconclusive`, not crash.
5. Missing reference snapshot behavior remains deterministic and stage-aware.

## Acceptance Examples

1. Running L1 probe mode emits `equivalence_stage: l1_bounded_formal` and `status: l1_bounded_inconclusive` when Kani is not available.
2. L1 receipt includes bounds like `rust_files<=N;functions<=M;mode=...` and configured `timeout_ms`.
3. Missing reference with L1 selected emits `reference_snapshot_missing` with backend `kani_rust_bounded`.

## Invariants

- Must always hold:
  bounded formal status is explicit about mode and limits.
- Must not regress:
  L1 output must not claim unrestricted semantic equivalence.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  semantic diff CLI adds stage/policy flags and L1 status labels
- Illegal states to remove:
  unbounded/implicit L1 runtime behavior

## Workload And Complexity Notes

- Workload shape and expected scale:
  bounded candidate selection + one Kani subprocess per request
- Hot, warm, or cold path:
  warm interactive path (opt-in)
- Dominant operations and expected frequency:
  set filtering of changed Rust files, bounded symbol selection, subprocess polling
- Chosen data structures and why:
  `BTreeSet` and `Vec` for deterministic bounded scope selection
- Key operation costs:
  `O(n log n)` scope selection over changed paths/symbols; subprocess runtime bounded by timeout
- Measured vs inferred performance claims:
  subprocess latency is measured by timeout-governed execution; broader formal cost remains workload-dependent

## Test Strategy

- Unit:
  L1 scope bounding test and status label mapping test
- Integration:
  L1 probe/check CLI smoke commands
- Regression:
  missing reference stage-aware receipt behavior

## Verification Notes

- Commands:
  `cargo test -p repobrain-cli`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode probe --l1-max-rust-files 6 --l1-max-functions 12 --l1-timeout-ms 3000 --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode check --l1-timeout-ms 1000 --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref DOES_NOT_EXIST --stage l1-rust-bounded --repo-root d:\RepoBrainOS`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
