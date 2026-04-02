# Verification Record: Semantic Diff Pipeline

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Scope

Validate the first deterministic semantic-diff implementation replacing the CLI placeholder with snapshot-vs-snapshot structural delta output.

## Intent / Spec References

- Intent: [Semantic diff pipeline intent](d:/RepoBrainOS/plans/2026-04-01-semantic-diff-pipeline-intent.md)
- Spec: [Semantic diff pipeline spec](d:/RepoBrainOS/plans/2026-04-01-semantic-diff-pipeline-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-cli`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- scan --revision HEAD --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`what-changed-semantically` now computes deterministic snapshot deltas instead of returning `not_implemented`.

Observed behavior:

- when reference snapshot is missing, status is `reference_snapshot_missing` with guidance to run `scan --revision <ref>`
- when reference snapshot exists and no structural differences are found, status is `no_structural_change`

The output now includes changed scope and delta dimensions:

- file adds/removes/metadata-modifies
- symbol fact adds/removes
- import fact adds/removes
- verification target adds/removes

## Pass / Fail Against Expectations

Pass.

The command now provides deterministic structural semantic-delta guidance and clear missing-reference recovery behavior.

## Performance / Complexity Validation

- Workload exercised:
  CLI unit tests with temp repos, plus repo-local smoke commands
- Measured:
  all tests and smoke commands executed successfully
- Inferred:
  diff complexity is bounded by map/set operations over two snapshot vectors
- Why no benchmark was needed, if applicable:
  this first slice is deterministic CLI logic and does not add new hot-path serving infrastructure

## Residual Risks

- output remains structural delta, not semantic equivalence checking
- changed scope can still miss deeper indirect impact propagation not yet wired
- overlay-aware semantic diffing is still out of scope for this slice

## Related

- Research: [Semantic diff pipeline](d:/RepoBrainOS/research/2026-04-01-semantic-diff-pipeline.md)
- SDD / ADR: [SDD-002](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md), [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- Logs / Artifacts: `.repobrain/snapshots/worktree-inventory.json`, `.repobrain/snapshots/HEAD-inventory.json`
