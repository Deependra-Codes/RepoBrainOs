# Verification Record: Invariants Engine

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Scope

Validate the deterministic Invariants Engine slice covering:

- snapshot-bound invariant derivation in `repobrain-graph`
- population of `BlastRadiusReport.invariants` and `VerificationPlan.invariants`
- broker propagation to `do_not_break`
- graph and broker test coverage for invariant behavior

## Intent / Spec References

- Intent: [Invariants engine intent](d:/RepoBrainOS/plans/2026-04-01-invariants-engine-intent.md)
- Spec: [Invariants engine spec](d:/RepoBrainOS/plans/2026-04-01-invariants-engine-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-graph -p repobrain-broker`
- `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- list-invariants --scope RepositoryScanner --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`repobrain-graph` now derives deterministic scoped invariants from maintained snapshot facts:

- scoped anchor paths
- scoped anchor symbols
- scoped structural reference edges
- scoped required verification checks

Those invariants are deduplicated, deterministically ordered, and bounded by category caps. The same derived set is now written to both `BlastRadiusReport.invariants` and `VerificationPlan.invariants`.

Broker safe-edit compilation now carries this populated set into `do_not_break` through existing contract wiring, and broker tests explicitly assert this behavior.

CLI smoke output for `RepositoryScanner` now returns non-empty `invariants` and matching `do_not_break` entries derived from the ingest scope and required scoped checks.

## Pass / Fail Against Expectations

Pass.

The slice now provides concrete deterministic invariants for anchored scope while preserving existing structural and verification planning behavior.

## Performance / Complexity Validation

- Workload exercised:
  snapshot-backed blast-radius and get-brief tests with scoped Rust fixture repos plus full repo-native quality/check commands
- Measured:
  targeted Rust tests and full `xtask` guardrails passed after the invariant engine changes
- Inferred:
  invariant derivation remains bounded `O(k log k)` over small per-request candidate sets due capped `BTreeSet` assembly
- Why no benchmark was needed, if applicable:
  no new persisted indexes or unbounded traversals were introduced; this is bounded hot-path bookkeeping over already-maintained snapshot data

## Residual Risks

- invariants remain intentionally structural and may miss higher-level semantic constraints
- no per-invariant evidence-id attachment yet; evidence remains available at report level
- category caps can omit lower-priority invariants in very dense scopes

## Related

- Research: [Invariants engine](d:/RepoBrainOS/research/2026-04-01-invariants-engine.md)
- SDD / ADR: [SDD-007](d:/RepoBrainOS/docs/sdd-007-production-shape-and-v1-cut.md), [SDD-008](d:/RepoBrainOS/docs/sdd-008-trust-readiness-and-verification-model.md)
- Logs / Artifacts: `.repobrain/snapshots/worktree-inventory.json` and command outputs listed above
