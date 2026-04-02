# ADR-002: Reality-Check Corrections For V1

Status: Accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Context

The earlier architecture was directionally strong but still had five serious realism risks:

- cross-language schema drift
- overtrusting tree-sitter for semantic truth
- runaway background maintenance on large repo churn
- a vague coverage-audit concept that could become fake certainty or looping logic
- a discipline model that risked becoming safe but exhausting

These are not edge cases. They are the kinds of flaws that can kill adoption even when the headline idea is strong.

## Decisions

### 1. Schema bindings must be generated, not manually mirrored

Canonical contracts remain in `schemas/`, but Rust, Python, and TypeScript bindings should be generated from them and checked for drift in repo-native automation.

### 2. Tree-sitter is the syntax floor, not the semantic ceiling

RepoBrain will use tree-sitter for universal fast extraction, but macro-aware and type-aware semantic claims require language-specific semantic adapters or compiler-backed evidence.

### 3. Background maintenance must be budget-controlled

RepoBrain will not treat every file change as an invitation to do fine-grained refresh work.

It will:

- debounce and batch events
- use clocks and source-control-aware bulk-change modes
- promote large churn to coarse snapshot refresh behavior
- cap concurrency and protect foreground developer experience

### 4. Coverage audit is about observable slot completeness, not omniscience

Coverage audit should answer:

- do we have the observable evidence slots needed for a safe answer?

It should not pretend to answer:

- have we discovered every true invariant in the repository?

Second-pass retrieval runs only when required observable slots are still retrievable and the latency budget allows it.

### 5. Execution discipline is a harness behavior, not a user ritual

RepoBrain keeps the research, spec, type, and test discipline, but the harness should absorb as much ceremony as possible.

That means:

- trivial tasks stay lightweight
- bounded tasks can satisfy multiple internal steps in one run
- architecture-visible work produces durable artifacts
- the user should feel supported, not nagged

## Consequences

### Positive

- the architecture is more honest
- the local-first story is safer
- the retrieval pipeline is less likely to fake certainty
- the agent UX becomes stricter internally without becoming miserable externally

### Costs

- more tooling investment in schema compilation
- more complexity in extraction infrastructure
- more explicit maintenance scheduling logic
- more nuanced task classification in the harness

## Related

- [SDD-002](d:/RepoBrainOS/docs/sdd-002-stack-and-research-direction.md)
- [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- [SDD-005](d:/RepoBrainOS/docs/sdd-005-latency-first-serving-strategy.md)
- [SDD-006](d:/RepoBrainOS/docs/sdd-006-contract-compilation-and-semantic-extraction.md)
