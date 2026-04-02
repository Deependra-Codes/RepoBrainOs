# Spec: L3 SMT And Alive2 Obligation Hardening

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

L3 theorem execution had deterministic obligation flow but no solver-backed SMT lane, and Alive2 integration only probed tool availability.

## Scope

- add configurable theorem solver mode with SMT-LIB execution (`z3`) and timeout policy
- keep deterministic relational fallback when SMT is unavailable or inconclusive
- harden translation-validation lane from tool probe to per-obligation execution:
  - build per-obligation abstract LLVM IR artifacts
  - run Alive2 per aligned pair obligation
  - aggregate proved/refuted/inconclusive evidence deterministically
- preserve replay artifacts and bounded scheduling behavior

## Non-Goals

- unrestricted full-program theorem proving
- compiler-emitted LLVM IR extraction for every language/runtime path
- replacing bounded relational model with unbounded semantics

## Behavioral Requirements

1. L3 must support explicit solver configuration in CLI policy (`relational_heuristic` or `smt_z3`).
2. SMT lane must execute bounded obligation checks and produce proved/refuted/inconclusive certificate outcomes.
3. SMT fallback behavior must be explicit and deterministic.
4. Alive2 lane must run per obligation (not only `--version` probing).
5. Evidence receipts must remain machine-readable and bounded.

## Acceptance Examples

1. With aligned equal signatures and available `z3`, theorem certificates report `proved` for pair obligations.
2. With pair-level divergence, theorem certificates report `refuted` and include deterministic witness tags.
3. With Alive2 enabled and available, receipt bounds report per-obligation checked/proved/refuted/inconclusive counts.

## Invariants

- Must always hold:
  L3 results remain bounded by explicit policies and never claim unrestricted equivalence.
- Must not regress:
  missing toolchains must degrade to explicit inconclusive/fallback states, not crashes.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  `what-changed-semantically` adds theorem solver and translation-obligation policy flags
- Illegal states removed:
  Alive2 probe-only mode for the translation sidecar

## Workload And Complexity Notes

- Workload shape and expected scale:
  bounded obligation scheduling plus bounded external-process execution per obligation
- Hot, warm, or cold path:
  warm bounded CLI path
- Dominant operations and expected frequency:
  deterministic alignment reuse, SMT script generation, per-obligation tool invocation
- Chosen data structures and why:
  `BTreeMap`/`BTreeSet` for deterministic ordering and replay-stable evidence
- Main alternative considered:
  unconditional relational-only execution was rejected for solver-depth requirements
- Key operation costs:
  `O(n log n)` obligation preparation + bounded external command cost
- Memory / allocation notes:
  bounded temporary script/IR artifacts and small deterministic maps
- Measured vs inferred performance claims:
  inferred from bounded limits and deterministic loops

## Test Strategy

- Unit:
  existing theorem/l2/l1 unit suite plus translation-sidecar regression test
- Integration:
  semantic diff CLI run with L3 stage and policy knobs
- Regression:
  solver/Alive2 missing environments must return bounded inconclusive/fallback outputs
- Property / invariant:
  replay-stable ordering and stage-bounded wording must remain deterministic

## Verification Notes

- `cargo test -p repobrain-cli`
- `cargo xtask fmt`
- `cargo xtask quality`
- `cargo xtask check`
