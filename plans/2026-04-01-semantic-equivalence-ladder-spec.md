# Spec: Semantic Equivalence Ladder Architecture

Status: accepted
Date: 2026-04-01
Owner: RepoBrain OS

## Intent Reference

- Intent: [Semantic equivalence ladder architecture intent](d:/RepoBrainOS/plans/2026-04-01-semantic-equivalence-ladder-intent.md)

## Problem Statement

Semantic-diff needed to move beyond placeholder L1/L2 language and deliver bounded but real execution: targeted L1 obligations and stronger L2 relational evidence without over-claiming full theorem-proving equivalence.

## Scope

- keep explicit equivalence contract + machine-readable evidence receipts in semantic-diff output
- harden L3 translation-validation sidecar:
  - attempt per-obligation Alive2 checks over compiler-emitted LLVM IR pairs when reference revision can be materialized
  - use snapshot-id revision hints when explicit `revision` metadata is missing
  - fall back to abstract-IR obligation checks only when compiler-emitted extraction is unavailable
  - report compiler-vs-fallback counts in receipt bounds and assumptions
  - support strict compiler-IR policy that prevents no-difference claims when fallback was used
  - support run-status override policy so translation counterexamples can force L3 `refuted_under_contract`
- harden L3 SMT lane beyond constant checks:
  - build weighted symbolic constraints from obligation-bound field tags (`contract_id`, `source_pair`, `assumptions`, `encoding_hash`) and aligned signal-field hashes
  - enforce a bounded semantic score threshold + mandatory equivalence predicates (`declaration`, `window_hash`, `behavior_signature`, import lane, line proximity)
  - emit score-aware assumptions/witness text so solver outcomes are traceable to bounded model semantics
- add a compiler-IR-first LLVM SSA/IVL backend for L3:
  - parse compiler-emitted LLVM functions into typed SSA blocks over integer/boolean/pointer state
  - symbolically execute supported acyclic control flow with phi/select/cast/load/store/call handling
  - encode return-state plus bounded observable effect summaries in QF_BV against canonicalized argument variables
  - fall back to the bounded signal-model lane only when compiler IR or supported SSA semantics are unavailable
- start a durable 5-iteration hardening pass across L0/L1/L2/L3:
  - publish the pass directly in `planned_backlog`
  - keep it wired in by default rather than hidden behind environment toggles
  - use it to stage future hardening without over-claiming completion
- harden L1 with harness-level obligation targeting:
  - discover `#[kani::proof]` harnesses in changed Rust scope
  - execute bounded per-obligation Kani runs
  - emit obligation-scoped witness text on failure/inconclusive outcomes
- harden L2 with deeper relational alignment:
  - add behavioral-profile alignment tier before name/similarity fallback tiers
  - enrich similarity scoring using declaration/import/context signatures
  - emit structured reason tags for relational mismatches
- harden L0/L2 baseline sensitivity by including file content hash in snapshot file metadata so same-size edits are still recognized as structural modifications

## Non-Goals

- claiming unrestricted full-program semantic equivalence across arbitrary languages
- replacing deterministic structural delta as the foundational stage
- forcing always-on heavy proving in interactive default flows

## Behavioral Requirements

1. `what-changed-semantically` must emit explicit equivalence contract fields and machine-readable receipts.
2. L1 check mode must prefer targeted harness obligations over workspace-wide blanket invocation.
3. L1 receipt bounds must include obligation budget context.
4. L2 must include behavioral-profile alignment and mismatch-reason witnesses.
5. L0/L2 must remain explicit about bounded, non-proof semantics.
6. Missing reference snapshot behavior remains deterministic with recovery guidance.
7. L3 translation lane must disclose whether checks used compiler-emitted LLVM IR or abstract fallback.
8. When translation run-status override is enabled, translation counterexamples must promote L3 run status to `refuted_under_contract`.
9. When strict compiler-IR mode is enabled, abstract fallback cannot finalize L3 no-difference status.
10. L3 SMT encoding must be symbolic over obligation and signal fields, not pre-resolved boolean constants.
11. L3 must try the LLVM SSA/IVL semantic-state backend before the signal-model SMT fallback when compiler-emitted IR is available.
12. L3 IVL summaries must account for bounded pointer, memory, and call effects when those instructions are in the supported subset.
13. `planned_backlog` must expose a 5-iteration L0/L1/L2/L3 hardening roadmap by default.

## Acceptance Examples

1. If snapshots are equal, output remains `no_structural_change` at L0 and bounded-no-difference at deeper stages.
2. If a changed Rust scope contains Kani proof harnesses, L1 executes targeted obligations and returns harness-scoped witness on failure/inconclusive status.
3. If function behavior context differs under aligned pairs, L2 witness includes explicit reason tags (for example declaration/import/context mismatch).
4. If same-size file contents differ, structural delta still marks file modification due content hash change.
5. If aligned pair field tags satisfy mandatory predicates and weighted score threshold, SMT lane returns `unsat` on negated equivalence formula (proved-under-bounds); otherwise `sat` yields refuted-under-bounds witness.
6. If compiler-emitted LLVM IR for an aligned pair is within the supported SSA subset, L3 proves/refutes over explicit symbolic return-state and bounded effect-summary formulas instead of hash-aligned signal tags.
7. If a function uses supported `ptr`/`load`/`store`/`call` patterns, IVL emits bounded pointer-memory-call assumptions instead of immediately abandoning the compiler-IR lane.

## Invariants

- Must always hold:
  equivalence receipts remain stage-bounded, deterministic, and machine-readable.
- Must not regress:
  no stage implies unrestricted semantic proof.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  semantic-diff output keeps contract/receipt fields and now carries richer L1/L2 witness semantics plus compiler-IR-first L3 assumptions
- Cross-crate type changes:
  `IndexedFile` now includes deterministic `content_hash` (serde-defaulted for backward compatibility)
- Illegal states removed:
  shallow L1 workspace-only invocation as default check strategy

## Workload And Complexity Notes

- Workload shape and expected scale:
  bounded set/map operations over snapshot facts + bounded per-obligation command execution
- Hot, warm, or cold path:
  L0 hot/interactive, L1/L2 warm/bounded by explicit policies
- Dominant operations and expected frequency:
  deterministic map/set diffing, harness discovery in changed Rust files, bounded alignment pairing, compiler-emitted LLVM extraction, bounded SSA symbolic execution
- Chosen data structures and why:
  `BTreeMap` / `BTreeSet` for deterministic ordering and reproducible evidence
- Main alternatives considered:
  unrestricted solver runs were rejected for latency and determinism risk
- Key operation costs:
  `O(n log n)` diff/alignment setup + bounded external process execution for L1 + bounded SSA path exploration / SMT encoding for supported L3 obligations
- Memory / allocation notes:
  bounded by snapshot-size maps and candidate pair/obligation limits
- Measured vs inferred performance claims:
  inferred complexity from bounded deterministic operations; no new perf benchmark in this slice

## Hardening Pass

The default staged hardening roadmap is:

1. L0 deterministic structural sensitivity + default verification guidance.
2. L1 obligation targeting, harness synthesis, and large-scope triage.
3. L2 alignment precision/recall and solver-backed relational witnesses.
4. L3 IVL memory/pointer/call summaries + stricter compiler-IR translation validation.
5. Cross-stage replay, perf ceilings, and no-overclaim audits.

## Test Strategy

- Unit:
  semantic delta, L1 scope bounding, L1 obligation discovery, L2 alignment/mismatch behavior, L3 signal-model SMT behavior, IVL parser/symbolic-execution/QF_BV encoding behavior
- Integration:
  semantic-diff CLI smoke on repo-local snapshots
- Regression:
  missing reference snapshot handling and no-overclaim wording
- Property / invariant:
  stage receipts never overstate proof guarantees

## Verification Notes

- Commands:
  `cargo test -p repobrain-ingest`
  `cargo test -p repobrain-cli`
  `cargo run -p repobrain-cli -- what-changed-semantically --ref HEAD --repo-root d:\RepoBrainOS`
  `cargo xtask fmt`
  `cargo xtask policy`
  `cargo xtask sync`
  `cargo xtask quality`
  `cargo xtask check`
- Artifacts:
  updated ingest file metadata, hardened semantic-diff implementation, and tests
