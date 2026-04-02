# Intent: L1 Rust Bounded Backend (Kani)

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

Semantic diff needs a first executable L1 backend for Rust that is bounded, timeout-controlled, and evidence-carrying.

## Why Now

L0 structural delta is complete. The next roadmap step is introducing bounded formal evidence without over-claiming full semantic equivalence.

## Goals

- add an L1 stage selector in `what-changed-semantically`
- add explicit L1 policy controls:
  - max rust files
  - max functions
  - timeout in milliseconds
  - Kani execution mode (`probe` or `check`)
- execute Kani through a timeout-governed runner
- emit machine-readable receipts with bounded policy + outcome status

## Non-Goals

- unrestricted proof obligations across all changed code
- cross-language formal proofs
- replacing L0 structural delta baseline

## Constraints

- keep CLI deterministic and bounded by explicit policy
- mark inconclusive outcomes honestly when Kani is unavailable or bounded run cannot conclude
- avoid claiming proof beyond configured bounds and mode

## Workload And Performance Shape

- Expected input size / scale path:
  bounded changed-scope Rust files and function candidates from snapshot structural delta
- Hot, warm, or cold path:
  warm interactive path (opt-in stage)
- Dominant operations and expected frequency:
  deterministic scope filtering plus one timeout-governed subprocess invocation
- Latency / throughput / memory sensitivity:
  latency-sensitive for CLI response; throughput is low-frequency interactive usage
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; first slice favors explicit bounded control and clear statuses over aggressive optimization

## Success Metrics

- `what-changed-semantically --stage l1-rust-bounded` runs and emits L1 receipts
- receipts include explicit bounds and timeout policy
- missing toolchain / timeout cases are represented as `inconclusive`, not hidden failures

## Verification Targets

- `cargo test -p repobrain-cli`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode probe --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode check --l1-timeout-ms 1000 --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`
