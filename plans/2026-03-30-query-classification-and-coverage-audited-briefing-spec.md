# Spec: Query Classification And Coverage-Audited Briefing

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Query classification and coverage-audited briefing intent](d:/RepoBrainOS/plans/2026-03-30-query-classification-and-coverage-audited-briefing-intent.md)

## Problem Statement

The current `BriefingPack` shape did not expose broker query classification or whether current evidence sufficiently covered the request type.

## Scope

- add `query_classification` and `coverage_audit` to `BriefingPack`
- update schema, Rust, TypeScript, and Python mirrors
- teach `repobrain-broker` to derive query classification and build required-slot coverage audits
- surface incomplete coverage in the final compiled brief

## Non-Goals

- adding dense retrieval
- adding new graph edge families
- extracting deterministic decisions in this slice

## Behavioral Requirements

1. `BriefingPack` must include a derived `query_classification`.
2. `BriefingPack` must include a typed `coverage_audit` with required slots, per-slot results, sufficiency, and summary.
3. `repobrain-broker` must derive query classification from the request deterministically.
4. `repobrain-broker` must audit only capability-matched slots in this slice.
5. Incomplete coverage must be surfaced in the briefing output, not hidden internally.

## Acceptance Examples

1. A normal safe-edit request over a scoped symbol returns `query_classification = safe_edit` with sufficient coverage.
2. A decision-style query over the same scope returns `query_classification = decision_why` and incomplete coverage because no deterministic decision evidence exists.
3. The briefing output remains evidence-backed and continues to include verification targets and flow summaries where available.

## Invariants

- Must always hold:
  - classification is derived, not user-invented inside the pack
  - coverage audit only claims slots supported by current deterministic capability
  - incomplete coverage remains explicit
- Must not regress:
  - exact anchor lookup and current `get_brief` behavior
  - contract sync across mirrors

## Contract And Type Changes

- Schema changes:
  - `briefing-pack.schema.json` adds `query_classification` and `coverage_audit`
- Public interface changes:
  - Rust, TypeScript, and Python contract mirrors expose the new audit types
- Illegal states to remove:
  - a briefing pack that looks complete but gives no typed statement about request coverage

## Workload And Complexity Notes

- Workload shape and expected scale:
  one audit over small broker-local vectors after exact and structural retrieval
- Hot, warm, or cold path:
  hot interactive broker path
- Dominant operations and expected frequency:
  request classification, required-slot selection, slot evaluation, brief compilation
- Chosen data structures and why:
  enums plus short vectors for required slots and slot results because the domain is small, typed, and latency-sensitive
- Main alternative considered:
  prose-only insufficiency notes were rejected because they are harder to validate and easier to drift
- Index ownership / maintenance notes:
  no new repo-scale indexes; audit consumes already-maintained snapshot and graph outputs
- Key operation costs:
  `O(k)` over required coverage slots, where `k` is small and bounded
- Memory / allocation notes:
  one extra audit object and a few short strings per briefing pack
- Measured vs inferred performance claims:
  inferred only; this slice is bounded bookkeeping layered on top of already-measured broker paths
- Benchmark expectation or reason none is needed:
  no additional benchmark is needed because the dominant work remains unchanged and `xtask perf` already covers `get-brief`

## Test Strategy

- Unit:
  domain and compiler compile against the new contract shape
- Integration:
  broker tests cover sufficient safe-edit coverage and incomplete decision coverage
- Regression:
  repo-wide sync and quality gates confirm mirror alignment
- Property / invariant:
  coverage audit required slots must correspond to the chosen query classification

## Verification Notes

- Commands:
  - `cargo test -p repobrain-domain`
  - `cargo test -p repobrain-compiler`
  - `cargo test -p repobrain-broker`
  - `cargo xtask perf`
  - `cargo xtask fmt`
  - `cargo xtask policy`
  - `cargo xtask sync`
  - `cargo xtask quality`
  - `cargo xtask check`
- Artifacts:
  - updated domain/schema/mirror contracts, broker classification and audit logic, and validation record
