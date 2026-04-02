# Intent: Semantic Diff Pipeline

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Problem Statement

`what-changed-semantically` is currently a placeholder response and does not compute any deterministic delta from maintained snapshot artifacts.

## Why Now

The pipeline readiness audit identified semantic diff as the first missing stage to implement. A deterministic first slice can improve change awareness without over-claiming full semantic equivalence.

## Goals

- replace the semantic-diff CLI stub with deterministic snapshot-to-snapshot structural delta output
- compare a target snapshot against a reference snapshot label passed via `--ref`
- emit scoped changed paths and fact-level delta counts (files, symbols, imports, verification targets)
- keep wording explicit that this is scoped structural delta, not exact semantic equivalence

## Non-Goals

- AST-level semantic equivalence checking
- overlay-aware unsaved buffer diffing
- automatic git traversal or revision snapshot generation
- LLM-based change interpretation

## Constraints

- Technical:
  use only stored snapshot artifacts and deterministic comparisons
- Product:
  avoid calling this full semantic understanding when only structural evidence exists
- Time / Team:
  ship a narrow reliable baseline that can be extended later

## Workload And Performance Shape

- Expected input size / scale path:
  two snapshot artifacts loaded from `.repobrain/snapshots` and compared in-memory
- Hot, warm, or cold path:
  interactive but bounded CLI query path
- Dominant operations and expected frequency:
  set/map creation over file paths, symbol fact IDs, import fact IDs, and verification target renderings
- Latency / throughput / memory sensitivity:
  interactive latency-sensitive, but linear scans over snapshot vectors are acceptable
- Acceptable simplicity-over-speed tradeoff, if any:
  yes; explicit set diff logic is preferred over complex incremental structures
- Likely failure mode at 10x scale:
  payload verbosity if changed-scope listing is not capped

## Success Metrics

- command no longer returns `status: not_implemented`
- command surfaces deterministic changed scope and fact delta counts
- missing reference snapshots are reported explicitly with recovery guidance
- tests cover no-change and changed-scope behavior

## Risks of Inaction

- pipeline remains blind to change deltas between snapshots
- users cannot quickly inspect what likely changed before safe-edit or explain-flow runs
- roadmap sequencing drifts because first missing stage remains unimplemented

## Research Scope

This slice is bounded to repo-local architecture and code:

- [SDD-002 stack and research direction](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md)
- [SDD-005 latency-first serving strategy](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- [Pipeline readiness gap map](d:/RepoBrainOS/research/2026-04-01-pipeline-readiness-gap-map.md)

No external source is required.

## Acceptance Shape

- Primary user-visible outcomes:
  `what-changed-semantically` emits deterministic structural delta instead of placeholder text
- Invariants that must remain true:
  output remains snapshot-bound and explicitly non-equivalence semantic language
- Verification targets:
  `cargo test -p repobrain-cli`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
