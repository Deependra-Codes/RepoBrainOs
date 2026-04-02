# Spec: Verification Target Extraction And Check Planning

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Verification target extraction and check planning intent](d:/RepoBrainOS/plans/2026-03-30-verification-target-extraction-and-check-planning-intent.md)

## Problem Statement

RepoBrain needs a deterministic first verification-planning slice that turns impacted files and symbols into concrete follow-up checks without executing anything on the user's behalf.

## Scope

- verification-target extraction during ingest from `Cargo.toml`, `package.json`, `pyproject.toml`, and common test layouts
- persisted verification targets in the maintained snapshot artifact
- blast-radius planning that emits required checks, recommended checks, coverage gaps, and stop conditions
- CLI output for the planned checks

## Non-Goals

- automatic command execution
- semantic test selection
- flaky-test handling
- broad build graph inference
- schema changes

## Behavioral Requirements

1. Repository scans must discover deterministic verification targets from supported repo manifests and layouts.
2. Extracted targets must remain snapshot-bound and be persisted in the local inventory artifact.
3. Blast radius must map impacted paths to required and recommended checks based on matching scope roots.
4. If no deterministic required checks are known for the impacted scope, the planner must emit a stop condition instead of implying coverage.
5. CLI blast-radius output must surface the planned checks and any coverage gaps.

## Acceptance Examples

1. If an impacted Rust file sits under a crate with `Cargo.toml`, blast radius returns `cargo test` as a required check for that crate scope.
2. If an impacted TypeScript file sits under a package with a `typecheck` script, blast radius returns `pnpm typecheck` for that package scope.
3. If blast radius hits files with no deterministic scoped check, the planner emits a coverage gap and a stop condition.

## Invariants

- Must always hold:
  verification planning remains recommendation-only and snapshot-bound.
- Must not regress:
  the planner must not pretend that repo-wide quality commands fully cover missing scoped verification.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  ingest snapshots gain verification targets; graph blast-radius reports gain structured verification planning.
- Illegal states to remove:
  impacted scope with zero deterministic checks should not appear fully covered.

## Workload And Complexity Notes

- Workload shape and expected scale:
  a small set of manifest-derived verification targets matched against one-hop impacted paths
- Hot, warm, or cold path:
  extraction is cold/warm maintenance; planning is interactive but bounded by impacted path count and extracted target count
- Chosen data structures and why:
  snapshot-owned vectors keep extracted targets simple and serializable; set-based planning in graph keeps required and recommended checks deduplicated deterministically
- Key operation costs:
  extraction is linear in scanned manifests and relevant layout files; planning is linear in extracted targets times impacted-path matching
- Memory / allocation notes:
  verification targets are compact snapshot metadata; planning allocates only small deduplication sets per query
- Measured vs inferred performance claims:
  complexity claims are inferred from the bounded vector-and-set design; verification measured behavior through tests and smoke commands rather than dedicated benchmarks

## Test Strategy

- Unit:
  manifest parsing, verification-target extraction, and scope matching behavior
- Integration:
  blast-radius planning tests across impacted Rust workspace scopes
- Regression:
  older snapshot artifacts without verification targets still deserialize safely
- Property / invariant:
  planned checks remain deduplicated and deterministic for the same snapshot and target

## Verification Notes

- Commands:
  `cargo test -p repobrain-ingest -p repobrain-graph`, `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`, `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`, `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  updated snapshot JSON and a validation record
