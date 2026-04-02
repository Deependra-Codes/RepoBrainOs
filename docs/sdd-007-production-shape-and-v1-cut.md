# SDD-007: Production Shape And V1 Cut

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

RepoBrain should ship first as a **fast, evidence-backed repo sidecar for coding agents**, not as a general autonomous repo brain.

## Why This SDD Exists

A product can be architecturally ambitious and still die from product shape.

The main failure mode here is not only technical drift. It is building something that:

- sounds profound
- does too much
- feels slow
- is hard to trust
- and never becomes a daily tool inside an actual coding loop

This SDD compresses the earlier architecture into the strongest production-ready shape.

## Product Reframe

The winning product is not:

- a giant repo memory
- a general code graph viewer
- a fully autonomous coding platform
- an always-on whole-repo reasoner

The winning product is:

- a repo-sidecar that maintains fresh repository state
- a broker that accepts targeted coding questions
- a compiler that returns the smallest trustworthy brief for the current task

Short version:

**maintain deep state in the background, answer narrow coding questions fast**

## The Core User Promise

When a working AI asks:

- what should I know before editing this
- what is the blast radius
- which invariants matter
- why is this built this way
- what changed likely affects this area

RepoBrain should answer with:

- relevant files
- likely dependencies
- tests and verification targets
- fragile zones
- evidence and freshness
- readiness and snapshot scope
- explicit uncertainty where needed

If it cannot answer safely, it should abstain or narrow scope.

## Why People Would Actually Use It

The product becomes daily-use only if it reliably improves the coding loop in four ways:

### 1. Faster onboarding

An agent should not need to rediscover repo structure on every task.

### 2. Safer edits

The agent should get invariants, blast radius, and verification targets before it changes code.

### 3. Better small-model performance

Weaker models should become usable by receiving narrower, more explicit, and more grounded context packs.

### 4. Lower human prompt burden

Humans should stop repeatedly explaining the same architecture facts to every tool.

## Production Shape

## 1. Single-Agent First, Tools First

The default product shape should be:

- one working coding agent
- RepoBrain as its repo-intelligence tool layer

Not:

- multiple collaborating agents by default
- multi-agent orchestration as the core product

Reason:

- simpler systems are easier to evaluate
- simpler systems are faster to trust
- tool quality matters more than agent-count aesthetics

## 2. Repo-Scoped Sidecar, Not Global Brain

Operate per repo or per repo plus revision family.

Reason:

- simpler freshness model
- simpler caching
- natural sharding
- less chance of global-state confusion

## 3. Snapshot-Bound Serving

Every answer should be attached to a concrete snapshot plus optional local overlay.

Reason:

- trust
- freshness visibility
- reproducibility

## 4. Query Types Before Fancy Retrieval

The product should feel strong because it routes well, not because it runs one expensive retrieval policy for everything.

The first production query set should stay narrow:

- `get_brief`
- `blast_radius`
- `explain_flow`
- `list_invariants`
- `what_changed_semantically`

## 5. Exact + Lexical + Structural Is The Real Backbone

The product should win first on:

- exact anchors
- lexical retrieval
- bounded structure
- evidence assembly

Dense retrieval is optional uplift, not the identity of the system.

## 6. Evidence And Freshness Are Product Features

These are not internal implementation details.

Users need to see:

- where the answer came from
- how fresh it is
- how certain the system is

That is what makes RepoBrain feel safer than generic agent context.

## 7. The UX Must Feel Supportive, Not Ritualistic

RepoBrain should be strict internally but lightweight externally.

The user experience should feel like:

- helpful preflight context
- grounded warnings
- clear next files
- clear verification

Not:

- eight visible ceremony steps before every edit

## The Three Loops That Matter

## Loop 1: Maintain

Background work:

- detect changes
- update indexes
- update staleness
- refresh higher-level views selectively

## Loop 2: Retrieve

Interactive work:

- classify request
- bind snapshot
- exact plus lexical plus structure retrieval
- coverage audit
- one bounded second pass if needed

## Loop 3: Compile

Answer work:

- shape for task
- shape for model profile
- attach evidence
- attach verification targets
- pack to latency and token budget

If these three loops are strong, the product is strong.

## The V1 Cut

The first version should deliberately exclude anything that weakens usability or trust.

## Must Ship In V1

- local-first repo scan and snapshot store
- exact, lexical, and structural retrieval planes
- query-aware routing
- evidence receipts
- freshness labels
- readiness state and snapshot or overlay disclosure
- one bounded coverage audit
- one bounded second pass
- model-profile-aware briefing compiler
- verification planner outputs for safe-edit queries
- MCP or agent tool surface
- sync and quality enforcement
- eval harness for safe-edit and repo-understanding tasks

## Must Not Block V1

- dense retrieval everywhere
- runtime observability ingestion
- PR-decision mining everywhere
- multi-agent orchestration
- autonomous code execution
- hosted control plane
- graph database dependency as a requirement for local use

## V1 Success Criteria

Ship only if we can show all of these:

### 1. Interactive usefulness

- most `get_brief` and `blast_radius` requests feel fast enough for IDE use

### 2. Trustworthy output

- major claims carry evidence and freshness

### 3. Safe-edit uplift

- using RepoBrain materially improves safe-edit success versus baseline prompting

### 4. Honest failure behavior

- the system abstains or narrows scope instead of bluffing

## Production Kill Switches

If any of these happen consistently, the product shape is wrong and must be corrected before scaling:

- users prefer generic repo chat because RepoBrain feels slower or more annoying
- the system often returns stale confident briefs
- the model cannot translate briefs into safer edits
- the maintenance path degrades local developer experience
- the product requires too much manual setup for ordinary repos

## Tomorrow's Coding Priorities

Start coding in this order:

1. snapshot store and repo inventory
2. exact and lexical retrieval
3. structural adjacency tables
4. `get_brief`
5. `blast_radius`
6. evidence and freshness wiring
7. eval harness for safe-edit uplift

Do not start with:

- dense retrieval
- hosted services
- autonomy
- cross-repo reasoning

## Final Decision

RepoBrain should be built first as a **production-grade repo sidecar for coding agents**.

That is the strongest version of the idea because it is:

- easier to trust
- easier to evaluate
- easier to adopt
- and still fully aligned with the long-term repo cognition vision
