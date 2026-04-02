# Intent: Performance And Complexity Discipline

Status: draft
Date: 2026-03-30
Owner: RepoBrain OS

## Problem Statement

RepoBrain already says to think about data structures and performance, but the current standards and templates do not force workload shape, path hotness, or measured-vs-inferred performance reasoning to be written down consistently.

## Why Now

AI-assisted implementation can easily produce code that is locally correct yet structurally weak for scale. We need stronger durable guardrails before that drift becomes repo habit.

## Goals

- make workload and data-structure reasoning explicit in the repo doctrine
- allow deliberate simplicity on cold paths without allowing accidental slowness on important paths
- add template fields so future intent, research, spec, and verification artifacts capture the same performance questions

## Non-Goals

- a universal benchmarking requirement
- a new runtime dependency
- a full automated reviewer for performance quality

## Constraints

- Technical:
  keep the change documentation-first and dependency-free
- Product:
  preserve latency-first thinking without turning every task into benchmark theater
- Time / Team:
  make the rule strong enough to matter but light enough to use every day

## Workload And Performance Shape

- Expected input size / scale path:
  repo-scale scans, indexes, graph walks, and repeated interactive requests
- Hot, warm, or cold path:
  mixed; some paths are interactive hot paths while operator tooling remains cold
- Latency / throughput / memory sensitivity:
  high on interactive Rust paths, lower on one-shot operator flows
- Acceptable simplicity-over-speed tradeoff, if any:
  yes on cold paths when deliberate and documented

## Success Metrics

- the repo has a dedicated performance and complexity standard
- templates require workload and complexity notes
- agent-facing guidance explicitly distinguishes hot-path rigor from cold-path simplicity

## Risks of Inaction

- future changes may stay correct but choose weak structures by default
- hot-path complexity can drift without an explicit review hook
- performance claims can remain vague and unmeasured

## Research Scope

Use:

- official guidance on API and boundary quality
- official guidance on performance measurement discipline
- RepoBrain's existing latency and agent guardrail doctrine

## Acceptance Shape

- Primary user-visible outcomes:
  future engineering artifacts ask the right performance questions by default.
- Invariants that must remain true:
  the process stays lightweight for ordinary work and stronger for scale-sensitive work.
- Verification targets:
  standards and template updates plus repo-native validation.
