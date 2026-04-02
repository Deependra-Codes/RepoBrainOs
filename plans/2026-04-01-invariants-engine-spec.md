# Spec: Invariants Engine

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [Invariants engine intent](d:/RepoBrainOS/plans/2026-04-01-invariants-engine-intent.md)

## Problem Statement

The current graph planner emits empty invariants even when deterministic scope anchors and verification signals are available, which leaves safe-edit invariant guidance underpowered.

## Scope

- add deterministic invariant derivation in `repobrain-graph` from:
  - scoped anchor paths
  - scoped anchor symbols
  - direct structural reference edges
  - required verification checks
- populate `VerificationPlan.invariants` from that derivation
- populate `BlastRadiusReport.invariants` with the same derived set
- add tests proving deterministic and non-empty invariant output for representative scoped targets

## Non-Goals

- semantic invariant mining
- cross-snapshot invariant reconciliation
- introducing probabilistic ranking for invariants
- schema or public contract shape changes

## Behavioral Requirements

1. Invariants must be generated only from maintained snapshot-backed deterministic facts.
2. Invariants must be deduplicated and deterministically ordered for identical inputs.
3. The graph layer must set `BlastRadiusReport.invariants` and `VerificationPlan.invariants` to the same derived invariant set.
4. Invariant generation must remain bounded to avoid unbounded response expansion in hot-path queries.
5. If deterministic scoped anchors exist, invariant output must not be empty.

## Acceptance Examples

1. A scoped symbol blast radius emits a symbol-definition invariant and required-check invariant.
2. A scoped path blast radius emits an anchor-path invariant for the target path.
3. A blast radius with no required checks still emits structural/anchor invariants while preserving stop conditions.

## Invariants

- Must always hold:
  invariant generation stays snapshot-bound and structural.
- Must not regress:
  safe-edit outputs must not imply semantic certainty beyond deterministic evidence.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  no new fields; existing invariant fields become meaningfully populated
- Illegal states to remove:
  scoped anchored blast-radius report with empty invariant fields

## Workload And Complexity Notes

- Workload shape and expected scale:
  one scoped request over one-hop impacted nodes and a small verification target set
- Hot, warm, or cold path:
  hot interactive path
- Dominant operations and expected frequency:
  set insertion, deduplication, and bounded rendering per blast-radius call
- Chosen data structures and why:
  `BTreeSet<String>` for deterministic ordering and deduplication with low implementation complexity
- Main alternative considered:
  append-only vectors with post-sort dedup were rejected in favor of simpler direct set semantics
- Index ownership / maintenance notes:
  no new persisted indexes; consume existing snapshot-owned path/symbol/import/verification data
- Key operation costs:
  bounded `O(k log k)` insertion over a small per-request invariant candidate set
- Memory / allocation notes:
  one small temporary set per request; list sizes are capped by category
- Measured vs inferred performance claims:
  complexity is inferred from bounded-set operations; no standalone microbenchmark is introduced
- Benchmark expectation or reason none is needed:
  no dedicated benchmark is required because the change is bounded and covered by existing hot-path quality checks

## Test Strategy

- Unit:
  graph invariant derivation assertions on symbol and path scoped targets
- Integration:
  broker safe-edit test confirms `do_not_break` is populated from planner invariants
- Regression:
  no-required-check scenario still preserves stop-condition behavior while keeping invariants non-empty
- Property / invariant:
  report-level invariants and verification-plan invariants are equal for the same blast-radius call

## Verification Notes

- Commands:
  `cargo test -p repobrain-graph -p repobrain-broker`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
- Artifacts:
  updated graph and broker tests, plus a validation record for the Invariants Engine slice
