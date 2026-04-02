# Performance And Complexity Discipline

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## Why This Exists

RepoBrain should not ship code that is merely correct on tiny examples but structurally wrong for the real workload.

At the same time, performance discipline should not turn into premature optimization theater.

We need a rule set that prevents accidental bad asymptotics while still allowing simpler slower code on cold paths when that tradeoff is explicit.

## One-Line Policy

**Time, space, and data-structure quality are release criteria for scale-sensitive work. Accept deliberate slowness only when the path is cold and the tradeoff is written down.**

## Core Rules

### 1. Start with workload, not with a favorite container

Before choosing a structure, state:

- expected input size
- scale path if the feature succeeds
- read vs write vs scan vs update behavior
- whether the path is hot, warm, or cold
- latency, throughput, or memory sensitivity

### 2. Name the chosen structure and the reason

When the choice materially affects behavior, record:

- the chosen data structure
- the main alternative considered
- why the chosen structure fits the workload better

### 3. Write the operation-cost story before changing a scale-sensitive path

For hot, repo-scale, or repeated interactive paths, record:

- the dominant operations
- their expected frequency
- the target or acceptable cost shape
- the index, cache, or table that makes the cost shape possible

If you cannot justify the structure with workload reasoning, stop and research before coding.

### 4. No avoidable bad asymptotics on repo-scale paths

Do not accept:

- avoidable `O(n^2)` behavior in repo-scale loops
- repeated full rescans on interactive paths without a written reason
- extra allocation or indexing layers on hot paths without a concrete benefit

### 5. Memory and allocation behavior are first-class

Do not treat memory as an afterthought.

When scale matters, record:

- resident-state growth
- allocation behavior on the hot path
- cache-locality implications when relevant
- who owns maintaining any added index or cache

### 6. Measured beats guessed on hot paths

For performance-sensitive work:

- benchmark or profile before claiming a win
- say what was measured
- say what was only inferred
- precommit to the measurement method when practical
- avoid benchmark theater without a stable method or meaningful workload

### 7. Push repeated cost to boundaries when practical

Prefer:

- parsing once at boundaries
- normalized typed values inside the system
- precomputed structures for repeated hot lookups
- reverse indexes for interactive graph traversals when reverse-neighbor lookup is part of the workload

Avoid:

- reparsing or renormalizing the same loose data across layers
- paying repeated costs because the boundary contract stayed weak
- repeated whole-edge rescans on interactive graph paths when a maintained reverse index is practical

### 8. Simpler slower code is allowed on cold paths

A slower design can still be the correct design when:

- the path is infrequent or operator-only
- the scale is bounded
- the simpler code reduces chaos or bug surface
- the tradeoff is written down as deliberate rather than accidental

## Expected Artifacts For Scale-Sensitive Work

### Intent

Include:

- workload and scale assumptions
- hot/warm/cold path classification
- dominant operations and expected frequency
- memory sensitivity
- what happens if the feature grows 10x

### Research

Include:

- candidate structures or approaches
- candidate indexes or caches when repeated lookups matter
- dominant operations and cost centers
- time and memory tradeoffs
- measurement plan when the path is performance-sensitive

### Spec

Include:

- workload notes
- chosen structure
- main rejected alternative when the choice matters
- operation-cost table or equivalent cost summary
- index ownership and maintenance notes
- expected complexity on key operations
- measured vs inferred performance claims

### Verification

Include:

- workload exercised
- benchmarks or profiling when run
- explicit note when no benchmark was justified
- residual performance uncertainty

## Review Questions

Before accepting a scale-sensitive change, ask:

- Is this path hot, warm, or cold?
- What operation dominates the cost?
- Is the data structure choice explicit?
- Is there an avoidable worse asymptotic path?
- What is the memory and allocation story?
- Who owns maintaining any extra index or cache?
- Was the result measured or only reasoned about?
- If the code is slower but simpler, is that tradeoff intentional and documented?

## Decision

RepoBrain will treat time, space, and data-structure quality as first-class engineering discipline and as release criteria for scale-sensitive work: workload-first, explicit about tradeoffs, measured on hot paths, and honest when simplicity wins on cold paths.
