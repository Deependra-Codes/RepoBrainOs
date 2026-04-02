# Type, TDD, Spec, And Agentic Loop Research

Date: 2026-03-30
Status: Draft

## Research Question

How should RepoBrain combine type-driven development, TDD, spec-driven development, and agentic execution so that AI assistance increases rigor instead of reducing it?

## Main Conclusion

The strongest loop is:

- spec first for behavior
- types first for state and contracts
- tests first for proof
- agentic execution in small validated slices

## What The Sources Suggest

### 1. Types are not just checks, they are design tools

Edwin Brady's type-driven development work treats types as a way to guide implementation rather than merely validate it at the end.

Implication:

- define contracts and state shapes before spreading logic

### 2. Domain modeling gets stronger when illegal states are modeled away

The F# domain modeling literature strongly advocates making illegal states unrepresentable.

Implication:

- RepoBrain should use narrower enums, tagged unions, and validated value types whenever meaning matters

### 3. Parse, don't validate

Alexis King's essay argues that weak input should be converted into stronger typed structures at the boundary instead of being repeatedly validated later.

Implication:

- this is especially useful for requests, evidence records, freshness states, and compiled outputs

### 4. TDD works best as a design loop, not just a testing ritual

Fowler's TDD write-up emphasizes the design feedback loop created by very small test and implementation steps.

Implication:

- RepoBrain should use TDD most strongly for behavior, regressions, and boundaries

### 5. AI-assisted coding benefits from harnessed loops

OpenAI's public Codex material shows the value of clear harnesses, stable protocols, and test-backed evaluation around agentic coding.

Implication:

- the agentic loop should be bounded and verification-aware, not open-ended generation

## Design Conclusions For RepoBrain

### 1. Use spec-driven development for non-trivial changes

The spec should define:

- required behavior
- examples
- invariants
- contract impact
- test strategy

### 2. Use type-driven development before complex branching

Before large implementation work:

- tighten schemas
- model states explicitly
- remove ambiguous primitives when the distinction matters

### 3. Use TDD where behavior matters most

Especially for:

- bug fixes
- public contracts
- normalization logic
- retrieval ranking and packing boundaries
- freshness and latency routing

### 4. Use bounded agentic loops

Default loop:

1. read intent and spec
2. propose contract and type changes
3. add failing tests or executable examples
4. implement one bounded slice
5. run narrow checks
6. refactor
7. run repo-wide checks
8. record verification

### 5. Do not force the doctrine where it does not fit

Docs-only or mechanical changes do not need theater.

The doctrine should apply most strongly to non-trivial behavior and architecture work.

## Recommendation

Document this as a repo standard and embed it in:

- templates
- contribution flow
- repo-native automation

## Sources

- Type-Driven Development with Idris: https://www.manning.com/books/type-driven-development-with-idris
- Domain Modeling Made Functional: https://fsharpforfunandprofit.com/posts/designing-with-types-making-illegal-states-unrepresentable/
- Parse, don't validate: https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
- Test-Driven Development: https://martinfowler.com/bliki/TestDrivenDevelopment.html
- Unlocking the Codex harness: https://openai.com/index/unlocking-the-codex-harness/
- How OpenAI uses Codex: https://cdn.openai.com/pdf/6a2631dc-783e-479b-b1a4-af0cfbd38630/how-openai-uses-codex.pdf
