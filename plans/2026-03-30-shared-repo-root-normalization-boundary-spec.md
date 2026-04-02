# Spec: Shared Repo-Root Normalization Boundary

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Shared repo-root normalization boundary intent](d:/RepoBrainOS/plans/2026-03-30-shared-repo-root-normalization-boundary-intent.md)

## Problem Statement

RepoBrain needs one shared definition of a canonical repo root so platform-specific normalization does not drift across Rust crates.

## Scope

- a shared repo-root path API in `repobrain-domain`
- migration of current Rust callers to that API
- tests for canonicalization and Windows-prefix normalization
- a guardrail update in engineering standards

## Non-Goals

- broad path-utility extraction
- relative-path helper consolidation
- path caching
- schema changes
- new dependencies

## Behavioral Requirements

1. RepoBrain must expose one shared API for repo-root canonicalization in the lowest shared Rust crate.
2. The shared API must canonicalize existing paths and normalize repo-root display and identity details.
3. On Windows, repo-root normalization must remove a leading `\\?\` verbatim prefix.
4. `repobrain-ingest` and `repobrain-cli` must use the shared API rather than local copies.
5. The change must remain `std`-only and must not widen into a generic utility surface.

## Acceptance Examples

1. Resolving `.` through the shared API returns an absolute repo root.
2. On Windows, normalizing `\\?\D:\RepoBrainOS` returns `D:\RepoBrainOS`.
3. Both the CLI and ingest scanner produce the same normalized root string for the same repo.

## Invariants

- Must always hold:
  Repo-root canonicalization has one owning Rust implementation.
- Must not regress:
  Platform-specific path normalization must not diverge between callers.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  `repobrain-domain` exports shared repo-root canonicalization helpers.
- Illegal states to remove:
  Multiple crate-local implementations of repo-root normalization.

## Workload And Complexity Notes

- Workload shape and expected scale:
  one canonicalization per caller entry point rather than repeated inner-loop path manipulation
- Hot, warm, or cold path:
  cold boundary path
- Chosen data structures and why:
  plain `Path` and `PathBuf` values keep the boundary narrow and avoid introducing a broader utility abstraction
- Key operation costs:
  cost is dominated by operating-system canonicalization; normalization itself is constant work on the returned path string
- Memory / allocation notes:
  allocation is minimal and bounded to canonical path ownership
- Measured vs inferred performance claims:
  no benchmark claim is made; this slice is optimized for consistency and ownership rather than runtime win claims

## Test Strategy

- Unit:
  domain tests for canonicalization and repo-root normalization behavior.
- Integration:
  compile and smoke verification through ingest and CLI commands.
- Regression:
  duplicate local repo-root helpers are removed from the Rust callers in scope.
- Property / invariant:
  canonical repo roots stay absolute and normalized for shared callers.

## Verification Notes

- Commands:
  `cargo test -p repobrain-domain`, `cargo test -p repobrain-ingest`, `cargo run -p repobrain-cli -- scan --repo-root d:\RepoBrainOS`, `rg -n "normalize_repo_root|canonicalize_repo_root" src/rust/crates`, `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  updated standards text and a validation record
