# Spec: Performance And Complexity Discipline

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Performance and complexity discipline intent](d:/RepoBrainOS/plans/2026-03-30-performance-and-complexity-discipline-intent.md)

## Problem Statement

RepoBrain needs durable process guardrails that make workload shape, data-structure choice, and measured-vs-inferred performance reasoning explicit for scale-sensitive work.

## Scope

- a dedicated standards document for performance and complexity discipline
- updates to agent and quality doctrine to reference that discipline
- template updates for intent, research, spec, verification, and prompt scaffolding

## Non-Goals

- mandatory benchmarks for every change
- new runtime features
- automatic complexity analysis
- schema changes

## Behavioral Requirements

1. The repo must define a durable rule that avoids accidental slowness while allowing documented simplicity on cold paths.
2. Scale-sensitive work must record workload shape and hot/warm/cold classification.
3. Templates must ask for data-structure choice, complexity notes, and measured-vs-inferred performance claims.
4. Agent-facing doctrine must explicitly require stronger reasoning for performance-sensitive work.
5. The change must remain documentation-first and must not add new dependencies or runtime behavior.

## Acceptance Examples

1. A future spec for a retrieval change includes workload shape, chosen structures, and key operation costs.
2. A cold-path operator flow can choose a simpler slower design and document why that is acceptable.
3. A hot-path change distinguishes measured benchmark results from inferred complexity reasoning.

## Invariants

- Must always hold:
  avoidable bad asymptotics on repo-scale and interactive paths remain disallowed.
- Must not regress:
  performance discipline must not collapse into mandatory benchmark theater for every task.

## Contract And Type Changes

- Schema changes:
  None.
- Public interface changes:
  none in runtime code; documentation and templates gain explicit performance and complexity sections.
- Illegal states to remove:
  vague performance-sensitive changes with no written workload or tradeoff reasoning.

## Workload And Complexity Notes

- Workload shape and expected scale:
  repo-scale deterministic core plus mixed hot/cold operator and serving paths
- Hot, warm, or cold path:
  mixed; standards must work for both latency-sensitive and cold-path work
- Chosen data structures and why:
  documentation and template changes only
- Key operation costs:
  process overhead should stay low while requiring explicit reasoning where scale matters
- Memory / allocation notes:
  not runtime-applicable for this documentation change
- Measured vs inferred performance claims:
  the doctrine must require this distinction

## Test Strategy

- Unit:
  not applicable
- Integration:
  repo-native policy, sync, and quality checks
- Regression:
  standards and templates should contain the new sections and links
- Property / invariant:
  the doctrine must permit deliberate cold-path simplicity while preserving hot-path rigor

## Verification Notes

- Commands:
  `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
- Artifacts:
  new standard, updated templates, research, and validation record
