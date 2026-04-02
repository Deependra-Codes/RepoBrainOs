# Intent: Performance Release Gate And Workload-First Data-Structure Enforcement

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain already has a performance discipline, but the current repo contract still leaves too much room for AI-assisted work to hand-wave dominant operations, memory costs, and measured-vs-inferred status.

That is not strong enough for a repo that wants interactive paths to stay latency-safe and scale-sensitive code to stay structurally sound.

## Why Now

The repo has started growing real interactive graph and broker paths.

If performance expectations remain soft, weak data-structure choices can still land with polished prose and passing tests.

## Goals

- make time, space, and data-structure quality explicit release criteria for scale-sensitive work
- force workload, operation-cost, and memory reasoning into templates and AI-facing prompts
- enforce performance / complexity validation sections across verification records
- add repo-native policy checks that make this discipline durable

## Non-Goals

- adding a full benchmark harness in this slice
- requiring benchmarks for every non-trivial change
- changing runtime behavior directly

## Constraints

- Technical:
  - enforcement should be lightweight and machine-checkable where practical
- Product:
  - the repo should stay rigorous without turning cold-path work into benchmark theater
- Time / Team:
  - existing records should be backfilled only as far as needed to support the stronger policy

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale interactive paths with repeated lookup, traversal, parsing, and planning behavior
- Hot, warm, or cold path:
  warm policy path governing future hot and warm implementation paths
- Dominant operations and expected frequency:
  repeated authoring, review, and policy validation on every non-trivial change
- Latency / throughput / memory sensitivity:
  repo policy checks should stay fast; stronger runtime discipline should reduce future latency regressions
- Acceptable simplicity-over-speed tradeoff, if any:
  yes for policy implementation itself, as long as it stays deterministic and low-chaos
- Likely failure mode at 10x scale:
  AI-generated code keeps passing correctness checks while introducing weak structure choices that are only noticed later

## Success Metrics

- the performance standard becomes stricter and more explicit
- templates require workload, cost, and memory reasoning
- validation records all carry measured-vs-inferred performance sections
- `cargo xtask policy` enforces the new structure

## Risks of Inaction

- performance claims remain too easy to bluff
- vibe-coded changes can stay superficially clean while structurally weak
- hot-path issues get discovered late, after behavior is already built on them

## Research Scope

- performance engineering doctrine from official and primary sources
- workload-first data-structure selection guidance
- measurement discipline for hot-path claims

## Acceptance Shape

- Primary user-visible outcomes:
  stronger repo doctrine and stricter policy enforcement for performance-sensitive work
- Invariants that must remain true:
  cold-path work can still stay lightweight when explicitly justified
- Verification targets:
  `cargo xtask fmt`, `cargo xtask policy`, `cargo xtask sync`, `cargo xtask quality`, `cargo xtask check`
