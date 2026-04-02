# Verification Record: Query Classification And Coverage-Audited Briefing

Status: accepted
Date: 2026-03-30
Owner: RepoBrain OS

## Scope

Validate the broker planner upgrade covering:

- typed `query_classification` in `BriefingPack`
- typed `coverage_audit` in `BriefingPack`
- deterministic broker-side classification and required-slot auditing
- explicit insufficiency surfacing for unsupported request coverage such as decision evidence

## Intent / Spec References

- Intent: [Query classification and coverage-audited briefing intent](d:/RepoBrainOS/plans/2026-03-30-query-classification-and-coverage-audited-briefing-intent.md)
- Spec: [Query classification and coverage-audited briefing spec](d:/RepoBrainOS/plans/2026-03-30-query-classification-and-coverage-audited-briefing-spec.md)

## Commands / Checks Run

- `cargo test -p repobrain-domain`
- `cargo test -p repobrain-compiler`
- `cargo test -p repobrain-broker`
- `cargo xtask perf`
- `cargo xtask fmt`
- `cargo xtask policy`
- `cargo xtask sync`
- `cargo xtask quality`
- `cargo xtask check`

## Environment

- OS: Windows
- Toolchain: Rust, Python, Node, pnpm

## Results Summary

`BriefingPack` now includes `query_classification` and `coverage_audit`, and `repobrain-broker` fills those fields deterministically from the current request and retrieved evidence.

Safe-edit requests now report sufficient coverage when exact anchors, structural context, and verification targets are available. Decision-style queries now remain honest: the pack classifies them as `decision_why`, marks coverage incomplete when no deterministic decision evidence exists, and surfaces that insufficiency in the briefing output.

## Pass / Fail Against Expectations

Pass.

This slice makes the broker materially more planner-like without pretending the system already has richer decision extraction or dense retrieval.

## Performance / Complexity Validation

- Workload exercised:
  broker classification and slot-audit execution on snapshot-backed `get_brief` tests, explicit `xtask perf`, plus full repo-native validation
- Measured:
  `cargo xtask perf` remeasured `blast-radius` and `get-brief`; both stayed within the configured p95 thresholds after the broker audit additions
- Inferred:
  the added audit is `O(k)` over a small required-slot set and should not materially change current broker latency
- Why no benchmark was needed, if applicable:
  not applicable; the hot-path perf surface was rerun because `get-brief` changed

## Residual Risks

- decision evidence remains intentionally unsupported in the current deterministic slice
- classification heuristics are intentionally simple and may need refinement once more task types or CLI surfaces are added
- broker still does not perform second-pass widening or abstention beyond surfacing insufficiency

## Related

- Research: [Query classification and coverage-audited briefing](d:/RepoBrainOS/research/2026-03-30-query-classification-and-coverage-audited-briefing.md)
- SDD / ADR: [SDD-004](d:/RepoBrainOS/docs/sdd-004-retrieval-and-grounding-pipeline.md)
- Logs / Artifacts: targeted Rust tests and repo-native validation commands above
