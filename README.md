# RepoBrain OS

RepoBrain OS is a repository cognition and context uplift system for coding agents.

The repo is intentionally split by responsibility so we can grow for years without turning the codebase into a mixed-language mess:

- `src/rust/` holds the deterministic systems core
- `src/python/` holds research, evaluation, and optional ML helpers
- `src/ts/` holds MCP and integration surfaces
- `schemas/` holds canonical cross-language contracts
- `docs/` holds architecture and SDDs
- `research/` holds market, paper, and concept research

## Repo Map

```text
.
|-- docs/
|-- research/
|-- schemas/
|-- scripts/
|-- src/
|   |-- rust/
|   |   `-- crates/
|   |-- python/
|   `-- ts/
`-- README.md
```

## Architecture Anchors

- [SDD-001](docs/sdd-001-repository-cognition-engine.md) defines the repository cognition and context uplift engine.
- [SDD-002](docs/sdd-002-stack-and-research-direction.md) defines the stack and research direction.
- [SDD-004](docs/sdd-004-retrieval-and-grounding-pipeline.md) defines the retrieval and grounding pipeline.
- [SDD-005](docs/sdd-005-latency-first-serving-strategy.md) defines the latency-first serving strategy.
- [SDD-006](docs/sdd-006-contract-compilation-and-semantic-extraction.md) defines contract compilation and syntax-vs-semantic extraction boundaries.
- [SDD-007](docs/sdd-007-production-shape-and-v1-cut.md) defines the production-ready product shape and v1 cut.
- [SDD-008](docs/sdd-008-trust-readiness-and-verification-model.md) defines trust boundaries, readiness states, overlay semantics, and verification planning.
- [SDD-003](docs/sdd-003-repo-layout-and-boundaries.md) defines repo layout, dependency boundaries, and ownership.
- [Engineering Quality Baseline](docs/standards/ENGINEERING_QUALITY_BASELINE.md) defines the enforced code-writing standard.
- [Docs Index](docs/README.md) gives the reading order and architecture map.

## Quick Start

Bootstrap the repo:

```powershell
./scripts/bootstrap.ps1
```

Repo-native quality commands:

```powershell
cargo xtask doctor
cargo xtask fmt
cargo xtask policy
cargo xtask sync
cargo xtask quality
cargo xtask check
```

Start every non-trivial task by reading:

1. [AGENTS.md](d:/RepoBrainOS/AGENTS.md)
2. [Docs Index](d:/RepoBrainOS/docs/README.md)

Manual setup, if you prefer it:

```powershell
python -m pip install -e .\src\python\repobrain_research[dev]
pnpm install
pnpm mcp:build
```

## Principles

- Deterministic extraction first.
- Retrieval is query-aware, bounded, and freshness-gated.
- Canonical contracts live in `schemas/`, not inside prompts.
- Research stays separate from the serving-critical core.
- Generated agent-facing files are views, not the source of truth.
- Every smart layer must be measurable with evals.
- AI-generated code is accepted only if it passes the same hard quality gates as human-written code.
- Non-trivial implementation should move through intent, spec, types, tests, implementation, and verification.
- Agents should research before writing non-trivial architecture, algorithm, or performance code.
- Canonical contracts, bindings, and repo indexes should stay machine-checked through `cargo xtask sync`.
- Non-trivial answers should disclose readiness, snapshot or overlay scope, and verification expectations.

## Repo-Native Automation

RepoBrain uses a repo-native automation surface through `cargo xtask`.

This exists so:

- local workflows and CI share the same entrypoints
- multi-language checks stay discoverable
- scripts remain thin wrappers instead of becoming hidden workflow logic
- the default path for humans and AI agents is the same quality baseline

## What Good Looks Like

We are aiming for:

- a low-friction local-first baseline
- honest uncertainty instead of fake semantic certainty
- clean contract boundaries across Rust, Python, and TypeScript
- an architecture that can scale up with optional accelerators instead of forcing heavyweight infrastructure on day one
