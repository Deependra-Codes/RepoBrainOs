# Verification Record: L2 Relational Semantic Backend

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Scope

Validate the first L2 relational semantic backend slice in semantic diff:

- stage selection (`l2_relational_semantic`)
- explicit L2 policy controls and deterministic multi-tier alignment
- bounded relational evidence receipts with witness data

## Intent / Spec References

- Intent: [L2 relational semantic backend intent](d:/RepoBrainOS/plans/2026-04-01-l2-relational-semantic-backend-intent.md)
- Spec: [L2 relational semantic backend spec](d:/RepoBrainOS/plans/2026-04-01-l2-relational-semantic-backend-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-cli`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l0-structural-delta --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l1-rust-bounded --l1-kani-mode probe --l1-timeout-ms 3000 --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref DOES_NOT_EXIST --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust

## Results Summary

`what-changed-semantically` now supports executable L2 relational mode with bounded controls:

- `equivalence_stage: l2_relational_semantic`
- backend `l2_relational_semantic`
- explicit bounds and timeout policy in L2 evidence receipts
- mismatch witnesses now include reason tags (for example declaration/import/context mismatch)

Observed outcomes:

- L0 stage remains deterministic and unchanged (`implemented` / `no_structural_change` shape preserved)
- L1 probe stage remains bounded and may return `l1_bounded_inconclusive` in current environment
- changed snapshots produce stage-scoped relational evidence with richer witness details
- alignment includes a behavioral-profile exact tier before same-name/signature fallback tiers
- structural baseline now detects same-size file edits via file `content_hash` in snapshot file metadata
- missing reference snapshot produces deterministic `reference_snapshot_missing` status and stage-aware recovery guidance
- L0 and L1 paths remain available and unaffected

## Pass / Fail Against Expectations

Pass for first L2 relational integration slice.

## Performance / Complexity Validation

- Workload exercised:
  bounded candidate construction + deterministic alignment + bounded pair comparison
- Measured:
  L2 commands complete and emit bounded evidence receipts
- Inferred:
  pair scoring cost scales with configured candidate cap; timeout policy provides upper bound for interactive latency

## Residual Risks

- L2 baseline currently uses snapshot-derived relational signatures, not solver-backed theorem proving
- alignment quality can degrade for large refactors with sparse import/symbol context
- when structural and relational signatures are unchanged, some outcomes remain bounded/inconclusive rather than proof-grade
- deeper cross-language semantic relation checks remain hardening backlog

## Related

- Research: [L2 relational semantic backend](d:/RepoBrainOS/research/2026-04-01-l2-relational-semantic-backend.md)
- Architecture: [Semantic equivalence ladder spec](d:/RepoBrainOS/plans/2026-04-01-semantic-equivalence-ladder-spec.md), [ADR-001](d:/RepoBrainOS/docs/adr-001-accuracy-first-v1-constraints.md)
