# Research: Semantic Equivalence Ladder

Status: draft
Date: 2026-04-01
Owner: RepoBrain OS

## Research Question

What architecture gets RepoBrain closest to practical semantic equivalence while staying honest, staged, and evidence-backed?

## Candidate Options

1. Keep structural diff only.
2. Attempt one-shot "full semantic equivalence" rollout.
3. Use a staged equivalence ladder (L0 structural -> L1 bounded formal -> L2 relational semantic + alignment).

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Structural only | Fast, deterministic, cheap | Weak semantic confidence ceiling | Users may over-interpret no-change output | Low | Reject as final state |
| 2. One-shot full equivalence | Strong headline ambition | Massive scope and tooling burden | High risk of over-claiming and schedule collapse | Very High | Reject |
| 3. Staged ladder | Honest claims with incremental strength | Requires staged design discipline | Medium integration complexity | Medium-High | Recommended |

## Primary Sources

- Kani (Rust bounded model checking):
  - <https://github.com/model-checking/kani>
  - <https://model-checking.github.io/kani/>
- Alive2 (LLVM optimization verification / translation-validation style):
  - <https://github.com/AliveToolkit/alive2>
  - Lopes et al., Alive2 PLDI 2021: <https://web.ist.utl.pt/nuno.lopes/pubs/alive2-pldi21.pdf>
- SymDiff (relational/paired semantic diff):
  - Lahiri et al., CAV 2012: <https://www.microsoft.com/en-us/research/publication/symdiff-a-language-agnostic-semantic-diff-tool-for-imperative-programs/>
- Differential equivalence checking / alignment inspiration:
  - DDEC OOPSLA 2013: <https://dl.acm.org/doi/10.1145/2509136.2509504>
  - KestRel (program relation checking): <https://arxiv.org/abs/2305.04745>
- Translation validation foundation:
  - Necula, PLDI 2000: <https://dl.acm.org/doi/10.1145/349299.349314>

## Findings

- Structural diff is valuable but cannot certify semantic equivalence.
- Bounded formal methods are strong when contracts and bounds are explicit.
- Relational checking can provide counterexample traces for changed paired procedures.
- Alignment quality is critical: name-only matching is too weak for refactors and reorganizations.
- Evidence receipts must carry assumptions and bounds, otherwise confidence is not auditable.

## Recommended Architecture

1. **Equivalence contract** (always required):
   explicit observable outputs, error behavior, side effects, preconditions.
2. **L0 structural backend** (always on):
   deterministic snapshot structural deltas with explicit non-proof status.
3. **L1 bounded formal backend** (opt-in / bounded):
   Rust via Kani; LLVM-level translation validation via Alive2-style checks.
4. **L2 relational backend** (opt-in / bounded):
   SymDiff-style paired-procedure checks with counterexample witnesses.
5. **Alignment engine**:
   DDEC/KestRel-inspired semantic matching for changed functions before L2 checks.
6. **Evidence receipts**:
   machine-readable records with backend, stage, solver, bounds, timeout, assumptions, witness.

## Direct Evidence vs Inference

- Direct:
  - L0 structural diff is implemented in current CLI.
  - the listed tools/papers support bounded formal or relational equivalence workflows.
- Inferred:
  - a staged ladder is the best fit for RepoBrain because it preserves low-chaos L0 behavior while adding higher-confidence modes incrementally.

## Unknowns / Follow-Ups

- precise IR boundary for Alive2-style checks in RepoBrain runtime
- first viable alignment features for semantically stable function pairing
- timeout and resource policy for solver-backed stages in interactive workflows

## Engineering Impact

- Contract / type impact:
  add explicit equivalence contract and evidence receipt types
- Testing impact:
  stage-specific correctness and non-overclaim regression tests
- Runtime / latency impact:
  L0 minimal impact; L1/L2 must remain bounded and typically off hot path
