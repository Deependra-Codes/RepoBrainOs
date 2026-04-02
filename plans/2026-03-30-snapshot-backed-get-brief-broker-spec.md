# Spec: Snapshot-Backed Get-Brief Broker

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Snapshot-backed get-brief broker intent](d:/RepoBrainOS/plans/2026-03-30-snapshot-backed-get-brief-broker-intent.md)

## Problem Statement

RepoBrain needs a real `get_brief` path that composes maintained snapshot state into a `BriefingPack` without pushing integration logic down into CLI-only code.

## Scope

- add a new `repobrain-broker` Rust crate with a snapshot-backed `get_brief` API
- reuse exact path and symbol anchors from ingest
- reuse one-hop structural adjacency and verification planning from graph
- reuse readiness, snapshot binding, and overlay disclosure from serving
- reuse packing and model-aware scaffolding from compiler
- expose the slice through `repobrain-cli get-brief`

## Non-Goals

- semantic retrieval or dense retrieval
- evidence receipt generation
- automated freshness inference beyond the provided readiness assessment
- repository-wide onboarding briefs without a scope anchor

## Behavioral Requirements

1. `get_brief` must require a scope hint and fail clearly when the scope cannot be anchored to an exact path or exact symbol in the current snapshot.
2. The broker must compile `must_know`, `suggested_files`, `impact_summary`, and `verification_plan` from maintained snapshot data only.
3. Unsupported higher-level fields such as `relevant_flows`, `relevant_decisions`, and evidence receipts must remain empty rather than being inferred speculatively.
4. The broker must bind the compiled brief to a concrete snapshot id, readiness state, and overlay scope.
5. CLI output for `get-brief` must be a machine-readable serialized `BriefingPack`.

## Acceptance Examples

1. A valid symbol scope such as `RepositoryScanner` returns a `BriefingPack` with exact-anchor guidance, direct structural neighbors, and package-scoped verification checks.
2. A valid path scope such as `src/rust/crates/repobrain-ingest/src/lib.rs` returns a `BriefingPack` anchored to that file and its direct neighbors.
3. A missing or unknown scope returns a bounded error instead of an empty or bluffing brief.

## Invariants

- Must always hold:
  the broker only composes maintained snapshot state and does not read arbitrary new repo state on the request path.
- Must not regress:
  CLI and future integration layers should not need to duplicate briefing-assembly logic.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  add `repobrain-broker` with a snapshot-backed `get_brief` API and add `repobrain-cli get-brief`
- Illegal states to remove:
  assembling a briefing pack from ad hoc CLI logic without a reusable broker boundary

## Workload And Complexity Notes

- Workload shape and expected scale:
  hot-path scoped lookups over one maintained snapshot and one bounded structural expansion
- Hot, warm, or cold path:
  hot interactive path over warm maintained data
- Chosen data structures and why:
  reuse sorted snapshot vectors for exact lookup and `BTreeSet`-based dedupe for small packing-stage collections to keep output deterministic
- Key operation costs:
  exact path lookup is `O(log n)`, exact symbol lookup is `O(log n + k)`, and packing work is linear in the small retrieved set
- Memory / allocation notes:
  the broker allocates only brief-sized temporary vectors and sets; it does not duplicate snapshot-scale state
- Measured vs inferred performance claims:
  interactive suitability is inferred from reuse of maintained indexes and bounded packing; no benchmark is added in this slice

## Test Strategy

- Unit:
  broker tests for exact-anchor briefing assembly, token-budget caps, and missing-scope errors
- Integration:
  CLI smoke commands against the real repo snapshot
- Regression:
  ensure verification targets and suggested files are preserved in the compiled brief
- Property / invariant:
  non-supported fields stay empty instead of carrying invented data

## Verification Notes

- Commands:
  `cargo test -p repobrain-broker`, `cargo run -p repobrain-cli -- get-brief ...`, and full `cargo xtask` validation
- Artifacts:
  repo-local snapshot artifact and validation record for this slice
