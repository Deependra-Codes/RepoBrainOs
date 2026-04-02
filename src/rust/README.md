# Rust Core

The Rust workspace holds the serving-critical and deterministic core:

- `repobrain-domain` for canonical Rust models
- `repobrain-ingest` for extraction and repository scanning boundaries
- `repobrain-graph` for graph-oriented queries and reports
- `repobrain-compiler` for context compilation and uplift policy
- `repobrain-serving` for latency-aware serving policy, routing, and cache keys
- `repobrain-cli` for local operator and developer workflows
- `repobrain-xtask` for repo-native automation
