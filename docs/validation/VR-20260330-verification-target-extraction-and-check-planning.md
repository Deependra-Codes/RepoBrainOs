# Verification Record: Verification Target Extraction And Check Planning

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the safe-edit planning slice covering:

- deterministic verification-target extraction from supported manifests and layouts
- persisted verification targets in the maintained snapshot artifact
- blast-radius planning for required and recommended checks
- CLI output for planned checks, coverage gaps, and stop conditions

## Intent / Spec References

- Intent: [Verification target extraction and check planning intent](d:/RepoBrainOS/plans/2026-03-30-verification-target-extraction-and-check-planning-intent.md)
- Spec: [Verification target extraction and check planning spec](d:/RepoBrainOS/plans/2026-03-30-verification-target-extraction-and-check-planning-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-ingest -p repobrain-graph`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

RepoBrain now persists deterministic verification targets in the snapshot and uses them during blast-radius planning.

The current slice discovers:

- scoped `cargo test` and `cargo check` targets for Rust package roots
- scoped `pnpm typecheck` targets for TypeScript package roots
- scoped Python unittest, mypy, and ruff targets when deterministic layouts are present
- repo-wide recommended quality commands when the repo exposes deterministic root-level quality entrypoints

Smoke validation on `d:\RepoBrainOS` produced a snapshot with 144 files, 300 symbols, 99 imports, and 21 verification targets. Running `blast-radius RepositoryScanner` returned the ingest structural neighborhood plus a required check of `cargo test (in src/rust/crates/repobrain-ingest)` and recommended checks of `cargo check (in src/rust/crates/repobrain-ingest)`, `cargo xtask check`, `cargo xtask quality`, and `pnpm quality:check`.

## Pass / Fail Against Expectations

Pass.

This slice turns blast radius into actionable safe-edit guidance without crossing the approval boundary into autonomous command execution.

## Performance / Complexity Validation

- Measured:
  ingest and graph tests plus CLI smoke commands were run successfully; no dedicated benchmark was run
- Inferred:
  extraction stays linear in the small manifest set, and planning stays bounded by impacted-path count and extracted verification-target count
- Why no benchmark was needed, if applicable:
  this slice adds bounded snapshot metadata and small planning sets rather than a new hot-path retrieval plane

## Residual Risks

- verification-target discovery is heuristic and intentionally conservative
- repo-wide recommended checks do not prove full scoped coverage
- Python and TypeScript target extraction is layout-driven rather than semantically resolved
- evidence receipts for why a specific check was selected are not yet emitted

## Related

- Plan: [V1 foundation plan](d:/RepoBrainOS/plans/v1-foundation-plan.md)
- SDD / ADR: [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md), [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- Logs / Artifacts: repo-local `.repobrain/snapshots/worktree-inventory.json`
