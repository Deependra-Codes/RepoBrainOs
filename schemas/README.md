# Schemas

This directory contains the canonical cross-language contracts for RepoBrain OS.

Rules:

- Change the schema first.
- Compile language-specific bindings from the schema second.
- Treat generated outputs like `AGENTS.md`, `CLAUDE.md`, and MCP responses as views derived from these contracts.

Important correction:

- handwritten Rust, Python, and TypeScript mirrors are a bootstrap state only
- the target architecture is generated bindings plus thin handwritten wrappers
- schema drift should become a repo-native build failure, not a review surprise

Current schemas:

- `model-profile.schema.json`
- `evidence-receipt.schema.json`
- `context-request.schema.json`
- `briefing-pack.schema.json`
- `impact-summary.schema.json`
- `snapshot-binding.schema.json`
- `overlay-scope.schema.json`
- `verification-plan.schema.json`
- `theorem-contract.schema.json`
- `theorem-proof-certificate.schema.json`
- `theorem-replay-artifact.schema.json`

Examples:

- `examples/model-profile.example.json`
- `examples/context-request.example.json`
- `examples/briefing-pack.example.json`
