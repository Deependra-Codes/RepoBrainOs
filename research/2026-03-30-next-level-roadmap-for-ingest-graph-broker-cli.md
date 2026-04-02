# Research: Next-Level Roadmap For Ingest, Graph, Broker, And CLI

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Research Question

What architecture upgrades would most strongly compound RepoBrain's four core crates:

- `repobrain-ingest`
- `repobrain-graph`
- `repobrain-broker`
- `repobrain-cli`

if the goal is an accuracy-first, latency-first, proof-carrying repository cognition system rather than a generic repo chat tool?

## Candidate Options

1. Keep the current lexical-first design and only add more heuristics.
2. Move to parser-backed structural extraction, but keep retrieval and packaging simple.
3. Build a staged compiler-grade pipeline: parser-backed facts, precise index import when available, typed multi-relational graph, query-aware broker, benchmark-first CLI.
4. Jump directly to a full Glean / CodeQL-class fact database and advanced learned retrieval stack.

## Evaluation Matrix

| Option | Benefits | Costs | Risks | Complexity | Verdict |
|---|---|---|---|---|---|
| 1. Heuristic expansion | Fastest to ship, low rewrite cost | Recall and trust ceiling stays low | Drift, brittle regexes, weak semantic signal | Low | Reject as long-term direction |
| 2. Parser-backed only | Big extraction quality gain, still local-first | Broker and graph still underpowered | Better facts without better planning still underdelivers | Medium | Useful but incomplete |
| 3. Staged compiler-grade pipeline | Best compounding path across all four crates | Requires disciplined intermediate representations | More moving parts, but still stageable | Medium-high | Recommended |
| 4. Full fact platform now | Highest long-run ceiling | Heavy infra and integration cost | Overbuild risk, slower product learning | Very high | Learn from it, do not copy all at once |

## Main Conclusion

RepoBrain should not try to become "a smaller Glean" or "a local Sourcegraph clone" in one jump.

The strongest path is a staged architecture that borrows the right ideas from compiler-grade code intelligence and repository-level retrieval research:

- parser-backed incremental extraction on the hot path
- precise semantic facts imported through standard code-intel formats when available
- a typed evidence graph with more than import edges
- a query-aware broker that plans retrieval by task type
- a CLI that is as much an evaluation and trace surface as an operator tool

In short:

**make `repobrain-ingest` more compiler-like, `repobrain-graph` more typed, `repobrain-broker` more planner-like, and `repobrain-cli` more benchmarkable**

## What The Strongest Sources Suggest

### 1. `repobrain-ingest` should stop at lexical extraction only as a temporary phase

Tree-sitter's official docs describe it as an incremental parsing library that can efficiently update syntax trees as files are edited and is fast enough for keystroke-scale usage. That is directly aligned with RepoBrain's latency-first local extraction model.

SCIP's official protocol and Sourcegraph's protocol notes point toward a stronger second tier: standardized, typed code-intelligence facts for definitions, references, and symbol relationships. Sourcegraph's official SCIP material explicitly frames it as simpler and more efficient than LSIF for code navigation and incremental indexing scenarios.

Glean and CodeQL show what the long-run ceiling looks like: store facts once, query them many ways, and keep syntax, semantic, and cross-file relationships available through stable schemas rather than re-deriving them ad hoc.

Implication for RepoBrain:

- keep lexical extraction only as bootstrap capability
- add parser-backed syntax facts as the default maintained layer
- add a second ingest lane that imports precise facts from SCIP where the language ecosystem already supports it
- normalize both lanes into one RepoBrain fact model instead of letting each language leak unique shapes upward

### 2. `repobrain-graph` should become a typed evidence graph, not just an import-neighborhood expander

CodexGraph argues that repository interaction benefits from structure-aware retrieval over similarity-only approaches.

DraCo shows that import-only retrieval is often insufficient and that dataflow-guided repo graphs can improve completion accuracy over state-of-the-art baselines.

RANGER goes a step further and separates entity queries from natural-language queries, using fast graph lookups for one and graph-guided exploration for the other.

Implication for RepoBrain:

- keep direct imports, but treat them as only one edge family
- add first-class edges for `defines`, `references`, `calls`, `tests`, `builds`, `documents`, and `supports_decision`
- attach freshness, provenance, and confidence to edges and nodes
- make `FlowCapsule` generation run over typed edge templates instead of a single structural summary pass

### 3. `repobrain-broker` should evolve from a packer into a retrieval planner

RepoCoder shows that iterative retrieval-generation beats one-shot repository retrieval.

MutaGReP is especially important for RepoBrain's product thesis: it shows that plan-grounded repository context can use less than 5% of a 128K window while rivaling or approaching full-repo context on hard tasks. That is a strong argument for coverage-aware, budget-aware briefing instead of "just stuff in more repo."

CodeRAG identifies three failure modes that match RepoBrain's future risks almost exactly:

- poor query construction
- single-path retrieval
- retriever / model misalignment

Citation-Grounded Code Comprehension adds the trust angle: hybrid retrieval plus graph expansion materially improves citation completeness and can reduce hallucinated grounding.

Implication for RepoBrain:

- classify requests before retrieval
- plan candidate gathering by task type
- use multi-path retrieval: exact, lexical, structural, and later dense
- perform a coverage audit before packing
- attach evidence receipts to every must-know claim and every flow summary
- abstain or widen scope when the coverage audit fails

### 4. `repobrain-cli` should become the proving surface, not just the utility surface

CrossCodeEval, RepoBench, DevEval, DependEval, FEA-Bench, and SWE-bench all reinforce the same product truth:

- repository tasks are much harder than single-file tasks
- cross-file dependency understanding is still weak
- feature implementation and safe multi-file edits remain difficult

This means the CLI should help answer:

- what was retrieved
- why it was retrieved
- what evidence supports it
- which checks were recommended
- how a new retrieval or graph policy changes task outcomes

Implication for RepoBrain:

- keep operator commands like `scan` and `get-brief`
- add trace and eval commands that let us compare retrieval policies and graph expansions
- make the CLI the fastest way to demo trust, not only the fastest way to inspect artifacts

## Recommended Architecture Direction

### `repobrain-ingest`

Target shape:

- `Tier 0`: deterministic file inventory and snapshot binding
- `Tier 1`: Tree-sitter-backed syntax facts for supported languages
- `Tier 2`: precise semantic fact import from SCIP when available
- `Tier 3`: optional heavyweight semantic enrichment off the hot path

Concrete upgrades:

- replace regex-style symbol extraction with parser-backed symbol, import, and module extraction
- add stable fact IDs so graph and broker layers stop rebuilding identities from strings
- support delta ingest by changed file set instead of full rescan as the default steady-state path
- store language facts in compact typed tables instead of only denormalized snapshot vectors
- keep lexical fallback for unsupported languages and explicitly mark it as weaker evidence

Why this is next-level:

- extraction quality rises without forcing full compiler integration on day one
- incremental parsing preserves latency safety
- standardized semantic import lets RepoBrain borrow mature language tooling instead of reimplementing compilers badly

### `repobrain-graph`

Target shape:

- graph nodes for files, symbols, modules, build targets, tests, docs, and decisions
- typed edges for ownership, import, reference, call, test coverage, build scope, and rationale support
- edge provenance and snapshot binding on every derived relationship

Concrete upgrades:

- replace path-only blast radius with weighted impacted entities
- add reverse lookup indexes per edge kind instead of repeated whole-graph scans
- compute `FlowCapsule`s from edge templates such as `entrypoint -> service -> persistence -> tests`
- represent verification reachability as graph facts instead of a last-minute string planner
- emit evidence receipts at edge construction time, not only during final report assembly

Why this is next-level:

- graph reasoning becomes composable
- blast radius can explain why a file, symbol, test, or build target is included
- future `explain_flow` and `blast_radius` become two views over the same fact graph

### `repobrain-broker`

Target shape:

- query classifier
- retrieval planner
- coverage auditor
- budgeted packer
- abstain / escalation path

Concrete upgrades:

- distinguish entity lookup, architecture explanation, safe edit, bug fix, feature implementation, and decision / why queries
- define required coverage slots per query type
- gather candidates through exact, lexical, structural, and optional dense channels
- rerank by evidence strength, freshness, and task relevance
- pack the smallest sufficient brief instead of the largest affordable brief
- emit explicit uncertainty when required slots are unfilled

Why this is next-level:

- small and mid-size models become much more useful
- trust is preserved because weak evidence can be surfaced as weak evidence
- the broker becomes the true product moat rather than a thin adapter

### `repobrain-cli`

Target shape:

- operator commands
- demo commands
- eval commands
- trace commands

Concrete upgrades:

- add `trace-retrieval` to show channels, graph expansions, reranking, and dropped candidates
- add `explain-flow` that reuses typed graph facts and evidence receipts
- add `eval retrieval`, `eval brief`, and `eval blast-radius` against benchmark-style local tasks
- add diffable JSON outputs so changes in ranking, coverage, and evidence can be regression tested
- add canned demos that prove the product loop end to end for one symbol, one bugfix, and one feature request

Why this is next-level:

- the CLI becomes the shortest path to learning whether architecture changes actually help
- demos stop being hand-wavy because the same interface can power both product storytelling and regression evaluation

## Recommended Build Order

1. Upgrade `repobrain-ingest` to parser-backed extraction with stable fact IDs.
2. Refactor `repobrain-graph` into typed edge families plus reverse indexes.
3. Teach `repobrain-broker` query classification and coverage slots.
4. Add CLI trace and eval surfaces before experimenting with dense retrieval.
5. Only then consider learned retrievers or heavier semantic indexing.

This order preserves RepoBrain's current values:

- local-first
- latency-safe
- evidence-backed
- progressive rather than overbuilt

## Recommendation

Adopt **Option 3**:

**a staged compiler-grade repository cognition pipeline**

More concretely:

- use Tree-sitter as the default maintained syntax substrate
- import SCIP where precise semantic facts already exist
- normalize all extracted facts into RepoBrain-owned typed contracts
- grow the graph into a multi-relational evidence graph
- turn the broker into a query-aware coverage planner
- turn the CLI into the canonical demo and eval surface

Do not:

- keep scaling lexical heuristics as the main architecture
- jump straight into a full Glean / CodeQL-class platform
- add dense retrieval before exact plus structural retrieval and coverage auditing are strong

## Direct Evidence vs Inference

- Direct:
  - Tree-sitter is built for incremental parsing and frequent updates.
  - SCIP is a typed code-intelligence protocol for definitions, references, and navigation.
  - LSIF exists as a workspace dump format, but SCIP's official material argues for simpler, smaller, and faster indexing ergonomics.
  - Glean and CodeQL both model code understanding as durable fact extraction plus structured querying.
  - RepoCoder, DraCo, CodeRAG, CoRet, RANGER, and MutaGReP all report that repository retrieval quality improves when structure, planning, or iteration is added.
  - CrossCodeEval, RepoBench, DevEval, DependEval, FEA-Bench, and SWE-bench all show repository-level work remains hard and cross-file understanding matters.
- Inferred:
  - RepoBrain's best moat is not any single extractor or retriever, but the broker that compiles evidence-backed task briefs.
  - A staged Tree-sitter plus SCIP path is likely the best latency / quality trade for RepoBrain's current maturity.
  - CLI investment is unusually high leverage because it can serve as operator surface, demo surface, and eval surface simultaneously.

## Unknowns / Follow-Ups

- Which supported languages should get native parser-backed extraction first after Rust, Python, and TypeScript?
- Should RepoBrain store the maintained fact model in JSON artifacts for longer, or move earlier to SQLite / RocksDB for incremental updates?
- How much semantic precision can be imported from existing language tooling without making onboarding too heavy for local-first use?
- Which benchmark mix best reflects RepoBrain's actual target product: repository QA, safe edit, feature implementation, or issue resolution?
- When dense retrieval is eventually added, should it operate over raw chunks, fact-backed summaries, or graph neighborhoods?

## Engineering Impact

- Contract / type impact:
  - add stable fact identifiers
  - add typed node and edge families
  - add query-type and coverage-slot enums in broker contracts
- Testing impact:
  - parser-backed golden tests
  - graph query fixtures
  - retrieval trace regression tests
  - benchmark harness and task suites in CLI
- Runtime / latency impact:
  - hot path should improve once incremental parsing replaces repeated regex scans
  - graph traversal latency should stay low if reverse indexes are maintained during ingest
  - broker latency will rise slightly with coverage audits, but should still beat larger-context prompting
- Workload assumptions:
  - read-heavy, change-local workloads
  - repeated queries against mostly stable repo snapshots
  - interactive safe-edit and explain-flow requests matter more than batch analytics
- Time complexity / constant-factor impact:
  - ingest should move from repeated full scans toward `O(changed_files)` steady-state updates
  - graph expansion should move from repeated global scans toward indexed neighborhood traversal
  - broker packing should stay bounded by candidate set sizes and budget caps, not repo size
- Memory / allocation impact:
  - typed fact tables and reverse indexes increase resident state
  - that trade is acceptable if it removes repeated recomputation on interactive paths
- Measurement plan or reason no benchmark is needed:
  - measure end-to-end `scan`, `blast-radius`, `get-brief`, and future `explain-flow` latency before and after parser-backed ingest
  - add benchmark tasks derived from repository retrieval and multi-file edit benchmarks
  - do not claim dense retrieval wins until compared against exact plus structural plus coverage-aware baselines

## Sources

- Primary:
  - Tree-sitter docs: https://tree-sitter.github.io/tree-sitter/index.html
  - SCIP protocol repo: https://github.com/sourcegraph/scip
  - Sourcegraph SCIP technical announcement: https://sourcegraph.com/blog/announcing-scip
  - LSIF protocol repo: https://github.com/microsoft/lsif-node
  - Glean official repo: https://github.com/facebookincubator/Glean
  - CodeQL official docs: https://codeql.github.com/docs/codeql-overview/about-codeql/
  - Kythe official repo: https://github.com/kythe/kythe
  - RepoCoder: https://aclanthology.org/2023.emnlp-main.151/
  - Dataflow-Guided Retrieval Augmentation for Repository-Level Code Completion: https://aclanthology.org/2024.acl-long.431/
  - CrossCodeEval: https://arxiv.org/abs/2310.11248
  - RepoBench: https://arxiv.org/abs/2306.03091
  - SWE-bench: https://arxiv.org/abs/2310.06770
  - DevEval: https://aclanthology.org/2024.findings-acl.214/
  - CodexGraph: https://aclanthology.org/2025.naacl-long.7/
  - CoRet: https://aclanthology.org/2025.acl-short.62/
  - DependEval: https://aclanthology.org/2025.findings-acl.373/
  - FEA-Bench: https://aclanthology.org/2025.acl-long.839/
  - CodeRAG: https://aclanthology.org/2025.emnlp-main.1187/
  - MutaGReP: https://arxiv.org/abs/2502.15872
  - RANGER: https://arxiv.org/abs/2509.25257
  - Citation-Grounded Code Comprehension: https://arxiv.org/abs/2512.12117
- Secondary:
  - none
