# Verification Record: L1 Rust Bounded Backend (Kani)

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Scope

Validate the first L1 bounded formal Rust backend slice in semantic diff:

- stage selection (`l1_rust_bounded`)
- explicit bounds and timeout policy inputs
- targeted `#[kani::proof]` obligation selection + Kani-backed bounded outcome labels

## Intent / Spec References

- Intent: [L1 Rust bounded backend intent](d:/RepoBrainOS/plans/2026-04-01-l1-rust-bounded-kani-backend-intent.md)
- Spec: [L1 Rust bounded backend spec](d:/RepoBrainOS/plans/2026-04-01-l1-rust-bounded-kani-backend-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-cli`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode probe --l1-max-rust-files 6 --l1-max-functions 12 --l1-timeout-ms 3000 --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode check --l1-timeout-ms 1000 --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref DOES_NOT_EXIST --stage l1-rust-bounded --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust

## Results Summary

`what-changed-semantically` now accepts L1 options and emits stage-aware receipts:

- `equivalence_stage: l1_bounded_formal`
- backend `kani_rust_bounded`
- explicit bounds and timeout policy in `evidence_receipts`
- check mode uses harness-level obligation targeting (manifest + harness scoped runs) when proof harnesses are discovered in changed Rust scope

Observed outcomes:

- probe mode remains toolchain-environment discovery oriented and may return `inconclusive`
- check mode now emits obligation-scoped witness identifiers for failures/timeouts/runtime errors
- missing reference snapshot produced deterministic `reference_snapshot_missing` status for L1 stage
- L0 path remains default and unchanged

## Pass / Fail Against Expectations

Pass for first L1 bounded integration slice.

## Performance / Complexity Validation

- Workload exercised:
  bounded scope selection + timeout-governed subprocess execution
- Measured:
  commands completed and emitted bounded receipts
- Inferred:
  check-mode runtime remains environment and workload dependent; timeout policy prevents unbounded waits

## Residual Risks

- Kani toolchain/environment availability can keep outcomes inconclusive
- proof harness discovery depends on lexical `#[kani::proof]` detection in changed Rust scope
- large change sets may exceed timeout budget before all obligations are attempted
- no unrestricted semantic equivalence claim is possible from this slice

## Related

- Research: [L1 Rust bounded backend (Kani)](d:/RepoBrainOS/research/2026-04-01-l1-rust-bounded-kani-backend.md)
- Architecture: [Semantic equivalence ladder spec](d:/RepoBrainOS/plans/2026-04-01-semantic-equivalence-ladder-spec.md), [ADR-001](d:/RepoBrainOS/docs/adr-001-accuracy-first-v1-constraints.md)
