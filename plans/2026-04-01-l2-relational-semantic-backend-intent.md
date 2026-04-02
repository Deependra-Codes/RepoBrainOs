# Intent: L2 Relational Semantic Backend

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

Semantic diff now has L0 structural and L1 bounded-formal coverage, but it still needs an L2 relational stage that aligns changed functions and emits bounded relational evidence.

## Why Now

The semantic-equivalence ladder is incomplete without an executable L2 stage. We need stage-complete behavior so users can request relational evidence directly from CLI.

## Goals

- add `l2-relational-semantic` stage selector in `what-changed-semantically`
- add explicit L2 policy controls:
  - max aligned pairs
  - max function candidates
  - timeout in milliseconds
  - alignment mode (`exact-only` or `signature-aware`)
  - minimum alignment score
- align changed function symbols deterministically (exact match first, then bounded similarity)
- emit machine-readable L2 receipts with relational outcomes and witness data

## Non-Goals

- unrestricted theorem-proving semantic equivalence
- always-on external solver orchestration in interactive path
- claiming global semantic equivalence when only bounded evidence is available

## Constraints

- keep L2 opt-in and bounded by explicit CLI policy
- keep deterministic ordering and deduplication
- keep uncertainty explicit (`inconclusive`) when no function evidence is available or timeout budget is exceeded

## Workload And Performance Shape

- Expected input size / scale path:
  bounded set of changed paths, bounded function candidate vectors, bounded pair comparison matrix
- Hot, warm, or cold path:
  warm interactive path (opt-in stage)
- Dominant operations and expected frequency:
  deterministic map/set construction plus bounded alignment scoring and pairing
- Latency / throughput / memory sensitivity:
  latency-sensitive for CLI responsiveness; memory proportional to bounded candidate vectors
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; first L2 slice favors deterministic explainability over aggressive optimization

## Success Metrics

- `what-changed-semantically --stage l2-relational-semantic` runs and emits L2 receipts
- receipts include explicit L2 bounds and timeout policy
- renamed/moved function alignment can still produce pair-level evidence under signature-aware mode

## Verification Targets

- `cargo test -p repobrain-cli`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
- `cargo run -p repobrain-cli -- what-changed-semantically --ref DOES_NOT_EXIST --stage l2-relational-semantic --repo-root d:\RepoBrainOS`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`
