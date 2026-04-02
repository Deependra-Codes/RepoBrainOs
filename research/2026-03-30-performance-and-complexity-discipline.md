# Performance And Complexity Discipline

Date: 2026-03-30
Status: Draft

## Research Question

What durable guardrails best prevent AI-assisted implementation from choosing weak data structures, accidental bad asymptotics, or unmeasured performance claims?

## Main Conclusion

RepoBrain should require workload-first performance reasoning rather than vague "optimize more" intentions.

That means:

- classify paths as hot, warm, or cold
- record chosen structures and main alternatives when scale matters
- forbid avoidable bad asymptotics on repo-scale paths
- allow simpler slower code only when the path is cold and that tradeoff is explicit
- distinguish measured performance claims from inferred ones

## What The Sources Suggest

### 1. Good boundaries create optimization room later

Abseil's performance guidance argues that APIs should preserve optimization opportunities instead of baking accidental cost and rigidity into the boundary.

Implication:

- RepoBrain should record performance-sensitive choices at the boundary level
- weak boundary decisions can make later optimization much harder

### 2. Measurement needs a declared method, not benchmark theater

Abseil's measurement guidance emphasizes stable methodology and clear interpretation instead of casual microbenchmark claims.

Implication:

- hot-path performance work should say what was measured and how
- unmeasured claims should be labeled as inference

### 3. Boundary validation and normalization are usually worth doing once

Rust API Guidelines recommend validating and typing data at boundaries so internal code stays predictable and cheap.

Implication:

- repeated parsing or normalization inside core paths is a design smell
- performance and correctness often improve together when the boundary is stronger

### 4. Simple design still treats duplication and excess elements as design failures

Beck's design rules, summarized by Fowler, keep "no duplication" and "fewest elements" as first-class design forces.

Implication:

- performance discipline should not become an excuse for speculative abstraction
- the target is explicit, workload-matched design, not complexity for its own sake

## Design Conclusions For RepoBrain

### 1. Add a dedicated performance and complexity discipline

The repo should explicitly require:

- workload shape
- hot/warm/cold path classification
- data structure choice
- time and memory tradeoffs
- measured vs inferred labeling

### 2. Make cold-path simplicity an allowed outcome

The right rule is not "always fastest."

The right rule is:

- no accidental slowness on meaningful paths
- simpler slower code is acceptable when the path is cold and the tradeoff is documented

### 3. Put the rule into templates, not just prose doctrine

If the repo wants the behavior to persist, the templates for intent, spec, research, verification, and prompt scaffolding should all ask for complexity notes in the same place.

## Recommendation

Add a dedicated performance and complexity standard, update the reusable templates to require workload and complexity notes, and make final verification explicitly distinguish measured performance from inferred claims.

## Sources

- Rust API Guidelines, Dependability: https://rust-lang.github.io/api-guidelines/dependability.html
- Abseil Fast Tip 79: https://abseil.io/fast/79
- Abseil Fast Tip 88: https://abseil.io/fast/88
- Martin Fowler, Beck Design Rules: https://martinfowler.com/bliki/BeckDesignRules.html
