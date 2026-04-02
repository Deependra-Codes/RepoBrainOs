# Spec: Pipeline Readiness Gap Map

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [Pipeline readiness gap map intent](d:/RepoBrainOS/plans/2026-04-01-pipeline-readiness-gap-map-intent.md)

## Problem Statement

Pipeline readiness is currently spread across source code and separate plan docs, making it hard to answer one practical question: what is still not set up.

## Scope

- add a docs-first readiness audit artifact with:
  - implemented pipeline stages
  - not-yet-setup stages
  - intentionally deferred stages
  - recommended implementation order
- anchor each gap to concrete source or executable output
- update repo indexes so sync checks remain green

## Non-Goals

- implementing any missing stage in this change
- adding new runtime commands or schema fields
- changing pipeline behavior

## Behavioral Requirements

1. The readiness document must include an explicit implemented/not-setup/deferred split.
2. Each not-yet-setup item must cite code paths or command outputs.
3. The recommended setup order must be bounded and stageable.
4. Documentation updates must pass repo sync and check gates.

## Acceptance Examples

1. Decision-evidence pipeline is listed as not set up with evidence from `decision_why` coverage audit showing missing `decision_evidence`.
2. Graph edge families beyond `defines` and `references` are listed as not set up with code anchors.
3. Overlay/readiness-aware CLI input support is listed as not set up with concrete call-site evidence.
4. Semantic-equivalence ladder stages beyond L0 are listed as not set up with evidence that only structural delta backend is active.

## Invariants

- Must always hold:
  readiness claims are evidence-backed, not aspirational
- Must not regress:
  docs must not imply features are implemented when code paths are placeholders

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  none
- Illegal states to remove:
  undocumented pipeline status ambiguity

## Workload And Complexity Notes

- Workload shape and expected scale:
  small static docs updates with bounded command verification
- Hot, warm, or cold path:
  cold documentation path
- Dominant operations and expected frequency:
  reading code, running smoke commands, writing markdown summaries
- Chosen data structures and why:
  markdown tables and bullet lists for audit readability
- Main alternative considered:
  embedding readiness notes across many docs was rejected due to drift risk
- Index ownership / maintenance notes:
  update `plans/README.md`, `research/README.md`, and `docs/validation/README.md`
- Key operation costs:
  negligible runtime cost; maintenance cost is review discipline
- Memory / allocation notes:
  none
- Measured vs inferred performance claims:
  none; this is documentation-only
- Benchmark expectation or reason none is needed:
  no benchmark needed for docs-only slice

## Test Strategy

- Unit:
  none
- Integration:
  CLI smoke commands to confirm declared gaps
- Regression:
  `cargo xtask sync` index consistency
- Property / invariant:
  no listed "implemented" item should be contradicted by current command output

## Verification Notes

- Commands:
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- blast-radius RepositoryScanner --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- explain-flow --flow-name RepositoryScanner --depth compact --repo-root d:\RepoBrainOS`
  `cargo run -p repobrain-cli -- get-brief --goal "why is RepositoryScanner designed this way?" --scope RepositoryScanner --task-type query --token-budget 4096 --repo-root d:\RepoBrainOS`
  `cargo xtask sync`
  `cargo xtask check`
- Artifacts:
  readiness audit research doc and verification record
