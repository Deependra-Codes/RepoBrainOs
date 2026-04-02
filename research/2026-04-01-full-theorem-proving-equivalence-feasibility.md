# Research: Full Theorem-Proving Equivalence Feasibility

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

What architecture is required to get as close as possible to full theorem-proving semantic equivalence in RepoBrain, and what hard limits must be explicit?

## Key Findings

1. Unrestricted program equivalence is undecidable.
2. Practical systems succeed by constraining the contract domain and proving obligation sets compositionally.
3. Translation validation and relational verification are required layers, not optional polish.

## Primary Sources

- Equivalence problem overview (undecidability context):
  https://en.wikipedia.org/wiki/Equivalence_problem
- Kani model checker documentation:
  https://model-checking.github.io/kani/
- SymDiff differential verification paper (Microsoft Research):
  https://www.microsoft.com/en-us/research/wp-content/uploads/2015/02/symdiff-resilience.pdf
- SMT-LIB standard (solver interchange contract):
  http://smt-lib.org/
- Alive2 translation validation project:
  https://alive2.llvm.org/

## Source-Derived Implications

- theorem-grade claims must be contract-scoped:
  preconditions, observable outputs, side effects, error behaviors
- equivalence proof should be obligation-based:
  pair procedures, modules, and effect summaries rather than monolithic whole-program solving
- proof pipeline needs CEGAR-style refinement:
  when solver returns spurious counterexamples, refine abstraction and retry
- translation-validation layer is critical for compiler-level trust:
  source-level equivalence without backend translation checks leaves a trust gap

## Architectural Decisions Inferred

1. Keep L0/L1/L2 as always-available lower stages.
2. Add solver-backed L3 stage for relational obligations and certificate generation.
3. Add translation-validation checks for Rust/LLVM paths where feasible.
4. Add proof replay artifacts to keep claims auditable and reproducible.

## Unknowns / Follow-Ups

- best intermediate verification representation for cross-language obligations
- solver portfolio strategy (Z3/CVC5/others) and timeout scheduling
- scalable decomposition strategy for repository-scale proof closure

## Confidence

- undecidability constraint: high confidence
- architecture recommendation: medium-high confidence
- expected implementation effort: high
