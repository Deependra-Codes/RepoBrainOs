# SDD-008: Trust, Readiness, And Verification Model

Status: Draft
Date: 2026-03-30
Owner: RepoBrain OS

## One-Line Decision

Every RepoBrain answer must be bound to an explicit trust boundary, readiness state, snapshot and overlay scope, and verification plan.

## Why This SDD Exists

RepoBrain can still fail even with strong retrieval and serving architecture if it:

- answers from stale or partial state without saying so
- silently mixes committed repo state with local overlays
- reads or surfaces content it should have excluded
- gives safe-edit advice without telling the agent how to verify it

That kind of system may look impressive in demos and still fail in real use.

## Product Thesis

RepoBrain should feel trustworthy because it is explicit about:

- what state it used
- what it did not use
- how ready that state is
- what must be checked before acting on the answer

Trust must be visible, not implied.

## Non-Goals

This SDD does not define:

- autonomous code execution
- general organization-wide permissions
- full security policy for hosted multi-tenant deployments

## Trust Boundary

### V1 Operating Boundary

RepoBrain should ship first as a read-mostly sidecar.

It may:

- read repository files under the configured repo root
- read build and test configuration
- read git metadata needed for snapshot binding and change detection
- read explicit architecture and standards docs inside the repo

It must not, by default:

- write source files as part of the RepoBrain product surface
- execute builds or tests on its own authority
- exfiltrate repo content to remote systems without an explicit user-controlled integration
- index known secret material or excluded generated dumps

### Approval Boundary

Any action outside passive repo understanding must be treated as a separate host action.

Examples:

- running tests
- editing files
- using networked semantic services
- exporting briefs outside the local environment

RepoBrain may recommend these actions, but it should not blur recommendation with execution.

### Exclusion And Redaction Defaults

V1 should support exclusion rules before indexing.

Default exclusions should include at least:

- `.git/`
- `node_modules/`
- build outputs and cache directories
- binary blobs and oversized generated artifacts
- `.env*`
- common secret and credential files

If excluded material may affect an answer, the system should say the answer was produced with exclusions in effect.

### Content-Based Redaction Limitation

Path rules are necessary, but they are not enough.

V1 should explicitly acknowledge that secret-like content can still exist inside otherwise normal source files, configs, tests, or docs.

Implications:

- path exclusions reduce risk but do not eliminate it
- RepoBrain should not claim complete secret safety from path filtering alone
- content-based secret detection is a later capability, not an implicit v1 guarantee

Until content-based redaction exists, the product should:

- label export and remote-sharing surfaces as higher risk
- prefer local-first handling for raw evidence
- keep raw evidence snippets bounded and necessity-driven

## Readiness State Model

Every answer should carry a readiness state.

### Required States

#### `cold`

No usable maintained snapshot exists yet.

Allowed behavior:

- answer only trivial direct lookups against currently visible files
- otherwise ask for warm-up or abstain

#### `warming`

Initial or recovery indexing is in progress.

Allowed behavior:

- serve narrow answers with explicit limits
- avoid repo-wide confidence claims

#### `ready`

Required retrieval planes for the request are current enough to answer normally.

Allowed behavior:

- serve full bounded briefings within the request's latency and freshness policy

#### `stale`

A usable snapshot exists, but refresh is pending for relevant areas.

Allowed behavior:

- serve only if freshness labels remain visible
- narrow scope or lower confidence where impacted

#### `degraded`

One or more retrieval planes or enrichers are unavailable.

Allowed behavior:

- answer from remaining planes
- explicitly mark missing capability

#### `bulk_refresh`

Large repo churn is being coalesced or replayed.

Allowed behavior:

- prefer coarse structural answers
- avoid precise blast-radius claims outside touched scope

#### `overlay_only`

The answer materially depends on local overlay or unsaved editor content not fully folded into maintained indexes.

Allowed behavior:

- answer about local touched scope
- avoid claiming repo-wide structural certainty from overlay-only state

## Readiness Transition Model

The readiness states are not just labels. They are a runtime state machine.

### State Transition Triggers

#### `cold -> warming`

Trigger:

- initial bootstrap starts
- recovery rebuild starts after local state loss or invalid cache state

#### `warming -> ready`

Trigger:

- required retrieval planes for the current request are materialized and current enough to serve normal bounded answers

#### `warming -> degraded`

Trigger:

- bootstrap completes only partially
- a required plane or enricher fails during warm-up

#### `ready -> stale`

Trigger:

- relevant files, symbols, or indexes are invalidated by a change
- freshness SLA for the relevant scope expires

#### `stale -> ready`

Trigger:

- affected scope is refreshed enough for the current request class

#### `ready -> bulk_refresh`

Trigger:

- large coalesced churn crosses a batching threshold
- checkout, merge, branch switch, or mass rewrite makes fine-grained freshness temporarily unreliable

#### `stale -> bulk_refresh`

Trigger:

- additional large churn arrives while scoped refresh is still pending

#### `bulk_refresh -> warming`

Trigger:

- coalesced replay completes, but the repo is still rebuilding enough state to answer normally

#### `bulk_refresh -> ready`

Trigger:

- coalesced replay completes and required planes are current enough again

#### `ready -> degraded`

Trigger:

- a required retrieval plane, semantic enricher, or local service becomes unavailable

#### `degraded -> ready`

Trigger:

- missing capability recovers and the required scope is current again

#### `degraded -> stale`

Trigger:

- capability recovers only partially and stale scope remains

#### `available state -> overlay_only`

Possible source states:

- `ready`
- `stale`
- `degraded`

Trigger:

- the answer materially depends on unsaved editor buffers or local overlay content not folded into maintained repo state

#### `overlay_only -> ready`

Trigger:

- the local overlay is persisted and refreshed into maintained state

#### `overlay_only -> stale`

Trigger:

- overlay is persisted but relevant maintained scope is still pending refresh

### Transition Precedence

When multiple conditions apply at once, readiness should resolve in this order:

1. `cold`
2. `warming`
3. `overlay_only`
4. `bulk_refresh`
5. `degraded`
6. `stale`
7. `ready`

This keeps the system from presenting a deceptively healthy state when a stronger constraint is active.

## Snapshot And Overlay Model

Every answer must bind to:

- a snapshot id
- a revision or worktree marker
- an overlay descriptor when local edits are involved

### Layers

#### Base Snapshot

The maintained local snapshot built from committed or saved repo state.

#### Worktree Overlay

Saved local changes that differ from the bound revision.

#### Buffer Overlay

Unsaved editor content supplied by the host tool.

### Overlay Rules

- repo-wide claims should come from the maintained snapshot plus validated saved overlays
- unsaved buffer overlays may adjust local file- or symbol-level guidance
- unsaved overlays should not promote broad architectural certainty
- responses should indicate whether a claim is `snapshot_confirmed`, `overlay_adjusted`, or `overlay_local_only`

## Verification Planner

Verification is not an optional note at the bottom of the answer.

It is a first-class subsystem that converts retrieved context into concrete post-brief checks.

### Inputs

- task type
- suggested scope
- retrieved evidence
- readiness state
- freshness state
- model profile

### Outputs

- required checks
- recommended checks
- invariants to preserve
- missing verification coverage
- stop conditions that should block a risky edit

### V1 Planner Categories

The planner should be able to emit at least:

- file or symbol-level invariants
- targeted unit or integration test suggestions
- build or typecheck targets when known
- risky side effects to recheck manually
- conditions that require narrowing scope or abstaining

### Readiness-Aware Planning

When readiness is not `ready`, the planner should tighten the answer.

Examples:

- `stale` may require re-reading changed files before edit advice
- `degraded` may omit dependency-wide claims and raise manual review items
- `bulk_refresh` should bias toward postpone-or-narrow guidance for broad edits
- `overlay_only` may recommend local checks only

### `bulk_refresh` Planner Rules

When readiness is `bulk_refresh`, the planner should behave conservatively.

It should:

- keep required checks focused on touched scope only
- avoid broad repo-wide verification claims
- add stop conditions for edits that depend on unresolved broader blast radius
- prefer "wait for refresh" over invented certainty

## Response Envelope Requirements

The minimum safe response envelope for non-trivial questions should include:

- snapshot identifier
- readiness state
- freshness summary
- exclusion or redaction notes when relevant
- evidence references for major claims
- verification targets or abstention reason

If these are not available, the system should prefer a narrower answer over a polished bluff.

### `bulk_refresh` BriefingPack Shape

When `readiness_state = bulk_refresh`, the `BriefingPack` should be constrained.

Expected behavior:

- `snapshot_binding` should point to the last stable base snapshot used for the answer
- `overlay_scope` should identify touched scope when known
- `must_know` should contain only coarse, high-confidence, touched-scope guidance
- `suggested_files` may be returned, but should stay narrow
- `impact_summary` may describe touched scope, but should avoid repo-wide semantic certainty
- `verification_plan.required_checks` should focus on touched scope
- `verification_plan.stop_conditions` should include at least one condition that blocks broad risky edits until refresh stabilizes
- `verification_targets` should remain actionable, but should not imply full repo coverage

What should not happen in `bulk_refresh`:

- confident repo-wide blast-radius claims
- precise dependency-wide edit safety claims
- broad invariants presented as fully current unless independently confirmed

## V1 Must Ship

The first production cut should include:

- readiness state on every non-trivial answer
- snapshot and overlay disclosure
- exclusion and redaction defaults
- verification planner outputs for `get_brief` and `blast_radius`
- explicit abstention behavior when trust preconditions are not met

## Metrics And Kill Signals

Track at least:

- stale-confident brief rate
- answers served while `degraded` or `bulk_refresh`
- overlay-local answers incorrectly treated as repo-wide
- verification-plan coverage rate for safe-edit tasks
- excluded-path hits that materially changed the answer shape

If these metrics trend poorly, the system is not trustworthy enough for daily use.

## Decision

RepoBrain will treat trust, readiness, overlay binding, and verification planning as first-class product architecture, not documentation garnish.
