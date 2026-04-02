# Intent: Full Theorem-Proving Equivalence Program

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

RepoBrain currently provides deterministic structural delta (L0), bounded Rust model-checking evidence (L1), and bounded relational semantic evidence (L2). This is useful but not equivalent to full theorem-proving equivalence.

## Why Now

Users explicitly need proof-grade confidence and counterexample-grade diagnostics, not only bounded confidence signals.

## Goals

- define a rigorous equivalence contract that can be proved or refuted
- move from bounded evidence to solver-backed relational proof obligations
- support compositional proofs over repository-scale changes
- emit machine-checkable proof certificates and replay metadata

## Non-Goals

- claiming unrestricted automatic equivalence for arbitrary programs without constraints
- sacrificing interactive path latency for default workflows
- replacing deterministic L0/L1/L2 stages that remain useful for daily work

## Constraints

- technical:
  preserve deterministic behavior and bounded policies in hot paths
- product:
  no over-claiming beyond proven scope and assumptions
- theory:
  unrestricted semantic equivalence is undecidable; proof claims must be contract-scoped

## Workload And Performance Shape

- expected input size / scale path:
  repository-scale, multi-file, multi-function changes
- hot, warm, or cold path:
  theorem-proving stays warm/cold by default; L0 remains hot
- dominant operations:
  obligation generation, SMT solving, CEGAR refinement, certificate generation
- acceptable tradeoff:
  slower latency is acceptable for explicit deep verification runs

## Success Metrics

- explicit theorem contract is present for each proof run
- proof outcomes are certificate-backed (`proved`, `refuted`, `inconclusive`)
- counterexamples are replayable and scoped to concrete obligations
- compositional proof coverage can scale beyond single-function checks

## Risks of Inaction

- users may over-trust bounded evidence as theorem-grade proof
- no path to high-assurance workflows for critical changes
- architecture may drift into larger but still shallow heuristics

## Acceptance Shape

- primary outcomes:
  solver-backed relational obligations + proof certificates integrated into RepoBrain pipeline
- invariants:
  claims remain bounded by contract, model assumptions, and solver semantics
- verification targets:
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
