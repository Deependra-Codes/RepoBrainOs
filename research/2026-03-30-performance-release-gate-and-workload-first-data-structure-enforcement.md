# Research: Performance Release Gate And Workload-First Data-Structure Enforcement

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Research Question

What durable guardrails best prevent AI-assisted implementation from shipping weak time complexity, weak space behavior, or poorly justified data-structure choices on repo-scale paths?

## Candidate Options

1. Keep performance guidance as prose-only doctrine.
2. Strengthen templates, but leave existing validation and policy mostly unchanged.
3. Treat time, space, and data-structure quality as release criteria for scale-sensitive work, and enforce that doctrine in templates, validation records, and `xtask policy`.
4. Require benchmarks for every non-trivial change, regardless of path sensitivity.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Prose-only doctrine | Lowest process overhead | Easy to ignore in practice | AI keeps generating weak structures with confident wording | Low | Reject |
| 2. Templates only | Better prompts and spec shape | Existing records and future drift still weakly enforced | Review load stays high | Low-medium | Better, but incomplete |
| 3. Release-gate discipline plus policy enforcement | Strong alignment across docs, prompts, and machine checks | Requires backfill and stricter policy | Some ceremony if applied carelessly | Medium | Recommended |
| 4. Benchmark everything | Maximum rigor in theory | Expensive and often fake for cold-path work | Benchmark theater, slower iteration | High | Reject |

## Main Conclusion

RepoBrain should treat performance and data-structure quality the same way it treats type and verification quality on scale-sensitive work: as release criteria, not optional cleanup.

That does not mean "benchmark every edit."

It means:

- workload-first reasoning is mandatory when scale matters
- time and space tradeoffs must be explicit
- dominant operations and maintained indexes must have an owner
- hot-path claims must be measured or labeled as inference
- validation records must say what workload was exercised and why a benchmark was or was not needed

This is the strongest point on the rigor-vs-chaos curve for an AI-assisted repo.

## What The Strongest Sources Suggest

### 1. API and boundary choices should preserve optimization room

Abseil's performance guidance emphasizes that APIs can accidentally lock in cost and block later optimization if they are shaped casually.

Implication:

- RepoBrain should force explicit reasoning about which structures and indexes are owned at the boundary
- repeated scans and repeated normalization are design failures, not only implementation details

### 2. Measurement discipline matters more than benchmark theater

Abseil's measurement guidance argues that methodology matters more than casual benchmark numbers.

Implication:

- RepoBrain should require measured-vs-inferred labeling
- hot-path claims should state the workload and method used
- cold-path work should be allowed to skip benchmarking when the reason is explicit

### 3. Boundary validation and typed values often improve both correctness and performance

The Rust API Guidelines recommend validating data at boundaries so internal code can stay predictable and cheap.

Implication:

- a strong boundary often removes repeated parsing, repeated normalization, and repeated scans
- performance discipline should be tied to ownership and contract shape, not only micro-optimization

### 4. Data-structure choice should start with the workload, not habit

Rust's collections documentation and the Rust Performance Book both reinforce a practical rule: choose structures based on the actual operations, cost centers, and memory behavior instead of defaulting blindly.

Implication:

- RepoBrain should ask for dominant operations, expected frequency, and memory / allocation notes
- operation-cost tables are a useful forcing function for AI-assisted implementation

## Design Conclusions For RepoBrain

### 1. Performance should be a release gate for scale-sensitive work

The standard should say explicitly:

- time, space, and data-structure quality are release criteria for scale-sensitive work
- weaker structures are not acceptable just because they are easy to generate

### 2. The repo should require an operation-cost story

For hot, repo-scale, or repeated interactive paths, the author should record:

- dominant operations
- expected frequency
- key operation costs
- chosen structure or index
- owner of index maintenance

### 3. Validation should always distinguish measured from inferred

Every verification record should carry:

- workload exercised
- what was measured
- what was inferred
- why no benchmark was needed, if benchmarking was skipped

### 4. AI-facing prompts should carry the same rule set

If the repo wants vibe coding to stay high quality, the AI contract must say:

- stop and research if you cannot justify the structure
- do not hand-wave memory or allocation behavior
- do not claim hot-path wins without measurement

## Recommendation

Adopt a stronger performance release-gate discipline across:

- `docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md`
- `AGENTS.md`
- AI-facing prompt templates
- intent / research / spec / verification templates
- `cargo xtask policy`
- historical verification records that currently lack measured-vs-inferred notes

## Direct Evidence vs Inference

- Direct:
  - Abseil advises shaping APIs so they preserve optimization opportunities and warns against misleading performance conclusions without sound methodology.
  - The Rust API Guidelines recommend validating at boundaries for dependable APIs.
  - Rust's collections guidance and the Rust Performance Book both support workload-matched structure selection and explicit attention to allocation and profiling.
- Inferred:
  - The best way to keep AI-assisted coding from producing weak performance decisions is to make workload, operation costs, and measured-vs-inferred status part of the repo's release gate.
  - A template-plus-policy approach is likely stronger than review-only discipline while still avoiding benchmark theater.

## Unknowns / Follow-Ups

- Should RepoBrain eventually maintain a dedicated benchmark harness for graph and broker hot paths?
- Which latency-sensitive commands should get explicit regression thresholds first: `scan`, `blast-radius`, `get-brief`, or future `explain-flow`?
- Which future edge families will need their own maintained indexes and policy checks?

## Engineering Impact

- Contract / type impact:
  - none directly in runtime contracts
  - stronger documentation and policy contract for scale-sensitive work
- Testing impact:
  - backfill validation records with performance / complexity sections
  - add policy checks for those sections
- Runtime / latency impact:
  - no direct runtime behavior change in this slice
  - indirect improvement by making weak structures harder to land later
- Workload assumptions:
  - repeated interactive repository queries matter more than batch analytics
  - repo-scale paths should be explicit about repeated scans, indexes, and memory growth
- Dominant operations and cost centers:
  - repeated lookup, traversal, parsing, ranking, and verification planning on interactive paths
- Candidate data structures / indexes:
  - sorted vectors for deterministic exact lookup
  - forward and reverse indexes for repeated graph traversal
  - typed boundary values to remove repeated normalization
- Main rejected alternative and why:
  - review-only discipline was rejected because it leaves too much room for AI-generated weak structure choices to slip through
- Time complexity / constant-factor impact:
  - policy and templates add negligible build-time overhead
  - they should reduce future accidental high-cost paths
- Memory / allocation impact:
  - no direct runtime change here
  - future work should now be forced to justify any extra resident index state explicitly
- Measurement plan or reason no benchmark is needed:
  - no benchmark was needed for this slice because it strengthens doctrine and enforcement rather than changing a runtime hot path

## Sources

- Primary:
  - Rust API Guidelines, Dependability: https://rust-lang.github.io/api-guidelines/dependability.html
  - Rust std collections overview: https://doc.rust-lang.org/std/collections/index.html
  - The Rust Performance Book, Profiling: https://nnethercote.github.io/perf-book/profiling.html
  - The Rust Performance Book, Heap Allocations: https://nnethercote.github.io/perf-book/heap-allocations.html
  - Abseil Fast Tip 79: https://abseil.io/fast/79
  - Abseil Fast Tip 83: https://abseil.io/fast/83
  - Abseil Fast Tip 88: https://abseil.io/fast/88
- Secondary:
  - none
