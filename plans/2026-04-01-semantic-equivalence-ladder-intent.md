# Intent: Semantic Equivalence Ladder Architecture

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

RepoBrain currently provides deterministic structural deltas (L0), but does not yet provide bounded formal or relational semantic equivalence evidence.

## Why Now

Users need a clear path toward stronger semantic confidence without pretending unrestricted full-program equivalence is solved today.

## Goals

- define an explicit equivalence contract:
  - observable outputs
  - error behavior
  - side effects
  - preconditions
- keep L0 structural diff as the default always-on baseline
- stage L1 bounded formal equivalence backends:
  - Rust-focused bounded proofs (Kani)
  - LLVM-level translation validation (Alive2-style)
- stage L2 relational semantic diff backend:
  - paired-procedure equivalence checks with counterexample traces (SymDiff-style)
- add semantic alignment for changed procedures/functions (DDEC/KestRel-inspired)
- emit machine-readable evidence receipts (backend, solver, bounds, timeout, assumptions, witness)

## Non-Goals

- claiming exact semantic equivalence for unrestricted multi-language repositories
- replacing deterministic snapshot structural guidance as the baseline
- introducing heavyweight mandatory infrastructure on interactive hot paths

## Constraints

- Technical:
  keep interactive CLI paths latency-safe and deterministic-first
- Product:
  confidence claims must be bounded by evidence stage and backend scope
- Time / Team:
  implementation must land in narrow slices with verification records

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale snapshots + bounded candidate function pairs
- Hot, warm, or cold path:
  L0 hot/interactive; L1/L2 warm/off-hot-path by default
- Dominant operations and expected frequency:
  deterministic diffing, bounded solver invocations, relational matching
- Acceptable simplicity-over-speed tradeoff, if any:
  yes for L1/L2 initial slices, as long as hot path remains protected

## Success Metrics

- every semantic-diff output includes an explicit equivalence contract and machine-readable evidence receipts
- L0 evidence remains explicit about non-proof status
- L1 and L2 are introduced behind bounded, evidence-carrying statuses

## Risks of Inaction

- structural deltas may be misread as stronger guarantees than they are
- no upgrade path exists for users who need formal or relational evidence
- architecture claims drift away from verifiable implementation

## Acceptance Shape

- Primary user-visible outcomes:
  semantic-diff output explicitly carries contract + evidence receipt fields
- Invariants that must remain true:
  no stage may over-claim proof scope beyond its configured bounds
- Verification targets:
  `cargo test -p repobrain-cli`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
