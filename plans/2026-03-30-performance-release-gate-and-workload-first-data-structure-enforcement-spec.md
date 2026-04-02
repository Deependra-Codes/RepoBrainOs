# Spec: Performance Release Gate And Workload-First Data-Structure Enforcement

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Performance release gate and workload-first data-structure enforcement intent](d:/RepoBrainOS/plans/2026-03-30-performance-release-gate-and-workload-first-data-structure-enforcement-intent.md)

## Problem Statement

The repo needs a stronger and more durable way to prevent AI-assisted changes from hiding weak time complexity, weak space behavior, or unjustified data-structure choices behind polished prose and passing tests.

## Scope

- strengthen the performance and complexity standard
- strengthen AGENTS and AI-facing prompt surfaces
- strengthen the reusable templates for intent, research, spec, and verification
- enforce performance / complexity sections across validation records
- enforce the stronger structure through `cargo xtask policy`

## Non-Goals

- adding runtime benchmarks in this slice
- enforcing benchmark thresholds for all commands
- changing product behavior directly

## Behavioral Requirements

1. The performance standard must state explicitly that time, space, and data-structure quality are release criteria for scale-sensitive work.
2. Templates must require dominant operations, memory / allocation notes, and measured-vs-inferred performance reasoning.
3. AI-facing instructions must tell agents to stop and research when they cannot justify a structure with workload reasoning.
4. Validation records must include `## Performance / Complexity Validation` with measured and inferred notes.
5. `cargo xtask policy` must fail when the strengthened standard, templates, or validation-record structure is missing.

## Acceptance Examples

1. A future scale-sensitive spec must include dominant operations, chosen structures, and memory notes before implementation spreads.
2. A future verification record must explicitly say what workload was exercised and what was measured versus inferred.
3. `cargo xtask policy` must reject a validation record that lacks the required performance / complexity section.

## Invariants

- Must always hold:
  - scale-sensitive work is documented with workload, structure, and performance reasoning
  - hot-path claims are either measured or labeled as inference
- Must not regress:
  - cold-path work must still be allowed to choose a simpler slower design when that tradeoff is explicit

## Contract And Type Changes

- Schema changes:
  - none
- Public interface changes:
  - stronger repo doctrine and policy expectations only
- Illegal states to remove:
  - validation records without explicit measured-vs-inferred performance notes

## Workload And Complexity Notes

- Workload shape and expected scale:
  repeated non-trivial repo changes where AI assistance can otherwise smuggle in weak performance reasoning
- Hot, warm, or cold path:
  warm policy surface governing future hot and warm runtime paths
- Dominant operations and expected frequency:
  template filling, policy validation, and review on every non-trivial architecture-visible slice
- Chosen data structures and why:
  existing markdown sections and string-based policy checks are sufficient because this slice is doctrine enforcement, not runtime hot-path execution
- Main alternative considered:
  review-only enforcement was rejected because it is too easy for weak structure choices to pass with confident wording
- Index ownership / maintenance notes:
  not applicable for runtime state in this slice; policy ownership stays in `xtask`
- Key operation costs:
  policy enforcement remains file-read plus substring checks over a small set of repo documents
- Memory / allocation notes:
  negligible relative to normal repo validation
- Measured vs inferred performance claims:
  measured: policy and full repo validation pass; inferred: stronger doctrine should reduce future weak runtime choices
- Benchmark expectation or reason none is needed:
  no benchmark is needed because this slice does not change an interactive runtime path

## Test Strategy

- Unit:
  rely on existing `xtask policy` failure behavior and repo command validation
- Integration:
  run the full repo-native validation loop
- Regression:
  ensure validation-record structure is enforced going forward
- Property / invariant:
  every validation record in `docs/validation/` must carry the performance / complexity section

## Verification Notes

- Commands:
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated standards, templates, AI-facing docs, policy checks, and backfilled verification records
