# Spec: Full Theorem-Proving Equivalence Program

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [Full theorem-proving equivalence intent](d:/RepoBrainOS/plans/2026-04-01-full-theorem-proving-equivalence-intent.md)
- Research: [Full theorem-proving equivalence feasibility](d:/RepoBrainOS/research/2026-04-01-full-theorem-proving-equivalence-feasibility.md)

## Problem Statement

RepoBrain needs a proof-grade semantic equivalence program that goes beyond bounded confidence signals while remaining explicit about undecidability and proof scope.

## Scope

- define an explicit theorem contract model for semantic equivalence runs
- compile aligned changes into solver-backed relational obligations
- execute obligations under bounded policy and emit certificate-grade outcomes
- support compositional closure across multiple obligations in one run
- produce machine-readable proof artifacts for replay and audit

## Non-Goals

- claiming unrestricted automatic equivalence for all programs/languages
- replacing L0/L1/L2 for interactive developer workflows
- single-shot monolithic whole-repo solving without decomposition

## Behavioral Requirements

1. The theorem stage must require an explicit equivalence contract.
2. Each obligation must report one of:
   - `proved`
   - `refuted`
   - `inconclusive`
3. Refuted obligations must include replayable counterexample metadata.
4. Final run status must aggregate obligation outcomes deterministically.
5. Output language must never claim global proof when any obligation is inconclusive.

## Architecture Requirements

### 1) Contract Layer

- typed contract object for:
  - observable outputs
  - error behavior
  - side effects
  - preconditions
  - environment model assumptions

### 2) Alignment And Decomposition Layer

- align changed procedures/modules into relational pairs
- partition obligations by strongly-connected components and call-graph boundaries
- allow independent proof attempts per partition

### 3) Obligation Compiler Layer

- compile each pair into a relational proof obligation (SMT/IVL target)
- include summaries for:
  - input domains
  - memory/state effects
  - exceptional behavior
  - termination assumptions

### 4) Solver Orchestration Layer

- deterministic scheduling over obligations
- timeout policies per obligation and per run
- CEGAR loop support for abstraction refinement

### 5) Certificate And Replay Layer

- persist:
  - solver metadata
  - assumptions
  - obligation encoding hash
  - proof outcome
  - counterexample witness when present
- support deterministic replay command for the same snapshot pair and config

## Acceptance Examples

1. If all obligations are proved, output status is `proved_under_contract`.
2. If one obligation is refuted, output status is `refuted_under_contract` and includes counterexample witness.
3. If at least one obligation times out, output status is `inconclusive_under_contract`.
4. If no explicit contract is provided, theorem stage refuses to run.

## Invariants

- Must always hold:
  every theorem claim is contract-bounded and certificate-backed.
- Must not regress:
  lower stages (L0/L1/L2) stay usable and do not over-claim proof guarantees.

## Contract And Type Changes

- Schema changes (planned):
  theorem contract and proof certificate schemas under `schemas/`
- Public interface changes (planned):
  semantic-diff theorem mode output with obligation-level certificates
- Illegal states to remove:
  proof-grade wording without obligation/certificate evidence

## Workload And Complexity Notes

- Workload shape and expected scale:
  many bounded obligations over aligned changed entities
- Hot, warm, or cold path:
  warm/cold verification path
- Dominant operations:
  obligation generation (`O(n log n)` alignment/decomposition), solver runs, CEGAR refinement
- Chosen data structures:
  deterministic `BTreeMap`/`BTreeSet` for stable ordering + explicit obligation graph structure
- Memory / allocation notes:
  proportional to obligation count and encoded formula size
- Measured vs inferred performance claims:
  inferred in this planning slice; empirical budgets required once solver backend lands

## Test Strategy

- Unit:
  contract validation, obligation compilation determinism, result aggregation
- Integration:
  theorem run over known equivalent and known non-equivalent fixtures
- Regression:
  proof wording and certificate completeness checks
- Property / invariant:
  deterministic obligation ordering and stable certificate IDs

## Verification Notes

- Commands:
  `cargo test -p repobrain-cli`
  `cargo test -p repobrain-ingest`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
- Artifacts:
  intent + research + this spec; implementation slices to follow

## Execution Order

1. Add theorem contract + certificate schemas and domain types.
2. Add theorem run mode with obligation generation only (no solver yet).
3. Add first solver backend for bounded relational obligations.
4. Add counterexample replay and proof artifact persistence.
5. Add translation-validation integration for Rust/LLVM paths.
