# Engineering Execution And Design Principles

Date: 2026-03-30
Status: Draft

## Research Question

What execution policy and engineering principles best prevent AI-assisted development from turning into structurally weak code?

## Main Conclusion

The right answer is not "be stricter in code review."

It is:

- explicit execution stages
- small, reversible implementation steps
- disciplined use of DRY and YAGNI
- contracts and tests before broad implementation
- repository-native enforcement for the parts that can be checked automatically

## What The Sources Suggest

### 1. YAGNI is a speed and clarity rule, not a minimalism slogan

Martin Fowler frames YAGNI as a reminder to avoid building features or flexibility before there is a real need.

Implication:

- speculative abstractions are a liability in an AI-heavy repo

### 2. Duplication should be removed at the level of knowledge, not by forcing generic helpers too early

Fowler's refactoring work treats duplicated logic as a design smell, but the answer is disciplined refactoring, not instant abstraction everywhere.

Implication:

- RepoBrain should target one source of truth for knowledge
- but still allow temporary duplication until the right boundary is visible

### 3. Simplicity in design still matters under AI assistance

The Beck design rules prioritize:

- passing tests
- revealing intent
- no duplication
- minimal elements

Implication:

- this maps well to AI work because it rewards small, verifiable steps rather than giant generated patches

### 4. Tests and refactoring are part of one loop

Fowler's TDD guidance emphasizes small cycles that keep design responsive to feedback.

Implication:

- execution policy should explicitly separate failing-example creation, minimal implementation, and refactoring

### 5. AI workflows need stable harnesses and verification loops

OpenAI's public Codex materials show the value of stable harnesses, controlled loops, and test-backed evaluation for agentic coding work.

Implication:

- RepoBrain should require AI work to pass through repo-native commands and verification, not freeform prompting alone

## Design Conclusions For RepoBrain

### 1. Non-trivial work needs explicit stages

Recommended default:

- intent
- research
- spec
- types and contracts
- tests
- implementation
- verification

### 2. DRY must be interpreted narrowly and carefully

Apply DRY to:

- schemas
- domain meaning
- canonical knowledge objects
- repeated business rules

Do not use DRY as an excuse to create:

- generic utilities with no stable domain
- large base classes
- cross-layer convenience modules

### 3. YAGNI should be a first-class AI guardrail

AI tends to overbuild:

- extension points
- helper layers
- configuration surfaces

So YAGNI should be explicit in the repo doctrine.

### 4. Enforcement should be partial but real

We can enforce:

- tool-based quality gates
- presence of key doctrine artifacts
- repo-native command surfaces

We cannot fully automate:

- good abstraction timing
- good boundaries
- good taste

So the doctrine must be both documented and partially enforced.

## Recommendation

Elevate RepoBrain from a quality baseline to an execution policy:

- document the execution stages
- add a spec template
- require `cargo xtask policy`
- tie contribution flow to intent, spec, tests, and verification

## Sources

- YAGNI: https://martinfowler.com/bliki/Yagni.html
- Beck design rules: https://martinfowler.com/bliki/BeckDesignRules.html
- Test-Driven Development: https://martinfowler.com/bliki/TestDrivenDevelopment.html
- Refactoring and duplication: https://martinfowler.com/books/refactoring.html
- Unlocking the Codex harness: https://openai.com/index/unlocking-the-codex-harness/
- How OpenAI uses Codex: https://cdn.openai.com/pdf/6a2631dc-783e-479b-b1a4-af0cfbd38630/how-openai-uses-codex.pdf
