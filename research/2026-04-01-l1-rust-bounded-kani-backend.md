# Research: L1 Rust Bounded Backend (Kani)

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

How should RepoBrain add a first bounded formal Rust backend (L1) while keeping claims explicit, latency controlled, and deterministic-first?

## Candidate Options

1. Add no L1 backend yet; keep only L0 structural evidence.
2. Run unbounded `cargo kani` and treat success as strong semantic proof.
3. Add a bounded and timeout-governed L1 mode with explicit inconclusive outcomes.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. L0 only | Simple and deterministic | No formal evidence path | Slower progress toward stronger guarantees | Low | Reject |
| 2. Unbounded L1 | Stronger signal when it finishes | Can be slow and noisy in large repos | Over-claim risk and poor UX on timeouts/tooling gaps | Medium | Reject |
| 3. Bounded + timeout L1 | Honest, controllable, stageable | Requires policy plumbing and receipt semantics | Inconclusive rates can be high initially | Medium | Recommended |

## Primary Sources

- Kani usage docs:
  - <https://model-checking.github.io/kani/usage.html>
- Kani proof harness model and attributes:
  - <https://model-checking.github.io/kani/reference/attributes.html>
- Kani CBMC argument model:
  - <https://model-checking.github.io/kani/cbmc-hacks.html>
- Kani CI notes (workspace/tests flags in practical usage):
  - <https://model-checking.github.io/kani/install-github-ci.html>

## Findings

- Kani verifies proof harnesses and can report success, failure, or resource-limited outcomes; this aligns with stage-bounded evidence rather than absolute semantic guarantees.
- `cargo kani` supports operational flags needed for an initial bounded integration strategy (`--tests`, `--harness`, unwind/CBMC controls).
- Bounded model checking naturally requires explicit limit framing; absence of counterexample under bounds should not be advertised as unrestricted equivalence.
- A host-side timeout policy is required for predictable interactive behavior in repo-scale workflows.

## Recommended L1 Policy

- Keep L0 structural diff as baseline and default.
- Add opt-in L1 stage (`l1_rust_bounded`) only when requested.
- Expose explicit L1 controls:
  - `max_rust_files`
  - `max_functions`
  - `timeout_ms`
  - `kani_mode` (`probe`, `check`)
- Emit machine-readable receipts with:
  - stage and backend
  - status (`no_difference_observed`, `observed_difference`, `inconclusive`)
  - bounds and timeout
  - assumptions and witness

## Direct Evidence vs Inference

- Direct:
  - Kani is harness-driven and bounded-model-checking based.
  - Kani CLI supports flags compatible with bounded workflows.
- Inferred:
  - for RepoBrain CLI, an explicit timeout-governed subprocess policy is the safest initial L1 integration pattern.

## Unknowns / Follow-Ups

- best obligation-selection strategy for mapping changed symbols to concrete Kani harness subsets
- when to add `--harness`-level targeting in check mode
- how to budget unwind/solver knobs without harming usability

## Engineering Impact

- Contract / type impact:
  no schema expansion needed beyond existing equivalence receipt fields
- Testing impact:
  add unit tests for bounded scope and status mapping
- Runtime / latency impact:
  subprocess execution is bounded by timeout policy, but check mode cost remains workload-dependent
