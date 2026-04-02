# Spec: Evidence Receipts And Structural Flow Capsules

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Intent Reference

- Intent: [Evidence receipts and structural flow capsules intent](d:/RepoBrainOS/plans/2026-03-30-evidence-receipts-and-structural-flow-capsules-intent.md)

## Problem Statement

RepoBrain needs a deterministic way to cite maintained evidence and a first honest flow abstraction so `get_brief` can explain why its scoped guidance exists.

## Scope

- add a snapshot capture timestamp to inventory snapshots
- emit deterministic evidence receipts from exact path hits, exact symbol hits, direct import edges, reverse import edges, and planned verification targets
- add a first `FlowCapsule` type in the structural layer for direct adjacency around a scoped target
- wire broker packing so `must_know.evidence_ids`, `relevant_flows`, and `evidence_index` become populated

## Non-Goals

- semantic flow understanding
- evidence snippets or hashes from source content
- cross-language schema changes
- explain-flow CLI or MCP implementation

## Behavioral Requirements

1. Every emitted evidence receipt must carry a deterministic id, source type, source ref, locator, and snapshot-bound capture time.
2. The first flow capsule layer must remain explicitly structural and direct-neighbor-based.
3. `get_brief` must populate `evidence_index` and attach evidence ids to supported `must_know` statements.
4. `relevant_flows` must remain empty only when no structural flow capsule can be derived for the scoped request.
5. Legacy snapshot artifacts without a capture timestamp must still deserialize safely.

## Acceptance Examples

1. A scoped symbol brief returns a symbol-definition receipt, import-edge receipts, verification-target receipts, and a structural flow summary.
2. A scoped path brief returns a file-anchor receipt plus direct structural neighbors and their receipts.
3. A legacy snapshot artifact without the new capture timestamp still loads, with a deterministic fallback timestamp value.

## Invariants

- Must always hold:
  evidence receipts and flow capsules come only from maintained snapshot facts and bounded structural expansion.
- Must not regress:
  unsupported semantic understanding must not be implied by structural flow wording.

## Contract And Type Changes

- Schema changes:
  none
- Public interface changes:
  add structural `FlowCapsule` types to the Rust graph layer and populate evidence fields in the broker output
- Illegal states to remove:
  evidence-backed claims with empty evidence ids when deterministic receipts exist

## Workload And Complexity Notes

- Workload shape and expected scale:
  one scoped request over one snapshot with one-hop structural expansion and small evidence/flow vectors
- Hot, warm, or cold path:
  snapshot timestamp creation is warm maintenance; receipt and flow assembly are hot but bounded
- Chosen data structures and why:
  use sorted `Vec` snapshots plus `BTreeSet`/ordered `Vec` dedupe to keep receipts and flow summaries deterministic
- Key operation costs:
  evidence assembly is linear in exact hits, direct imports, reverse imports, and selected verification targets for the scoped request
- Memory / allocation notes:
  the slice adds one timestamp string per snapshot and small receipt/flow vectors per request
- Measured vs inferred performance claims:
  bounded hot-path suitability is inferred from one-hop structural reuse; no dedicated benchmark is added

## Test Strategy

- Unit:
  ingest timestamp tests, graph receipt/flow tests, and broker evidence wiring tests
- Integration:
  CLI smoke command for `get-brief`
- Regression:
  legacy snapshot deserialization and evidence-id stability for representative inputs
- Property / invariant:
  structural flow wording stays structural and evidence-backed claims have non-empty ids when supported

## Verification Notes

- Commands:
  targeted crate tests, `cargo run -p repobrain-cli -- get-brief ...`, and full `cargo xtask` validation
- Artifacts:
  repo-local snapshot artifact and validation record for this slice
