from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal

ModelClass = Literal["weak_local_coder", "medium_cloud_coder", "frontier_agent", "custom"]
ScaffoldingLevel = Literal["minimal", "medium", "high"]
RequestDepth = Literal["compact", "standard", "deep"]
FreshnessRequirement = Literal["any", "fresh_preferred", "fresh_only"]
Freshness = Literal["fresh", "stale", "contradicted", "inferred", "unverified"]
LatencyClass = Literal["instant", "interactive", "deep", "background"]
ReadinessState = Literal[
    "cold",
    "warming",
    "ready",
    "stale",
    "degraded",
    "bulk_refresh",
    "overlay_only",
]
TaskType = Literal[
    "repo_onboarding",
    "safe_edit",
    "explain_flow",
    "blast_radius",
    "semantic_diff",
    "query",
]
QueryClassification = Literal[
    "entity_lookup",
    "architecture_explanation",
    "safe_edit",
    "bug_fix",
    "feature_implementation",
    "decision_why",
]
CoverageSlot = Literal[
    "exact_anchor",
    "structural_context",
    "flow_summary",
    "verification_targets",
    "impact_envelope",
    "decision_evidence",
]
CoverageStatus = Literal["satisfied", "partial", "missing"]
FreshnessImpact = Literal["none", "localized", "moderate", "high"]
OverlayKind = Literal["none", "worktree", "buffer", "mixed"]
OverlayClaimScope = Literal[
    "snapshot_confirmed",
    "overlay_adjusted",
    "overlay_local_only",
]


@dataclass(slots=True)
class ModelProfile:
    id: str
    model_class: ModelClass
    max_context_tokens: int
    preferred_scaffolding_level: ScaffoldingLevel
    notes: list[str] = field(default_factory=list)


@dataclass(slots=True)
class EvidenceReceipt:
    id: str
    source_type: str
    source_ref: str
    locator: str
    captured_at: str
    snippet_hash: str | None = None


@dataclass(slots=True)
class ContextRequest:
    goal: str
    task_type: TaskType
    consumer_type: str
    model_profile: ModelProfile
    question: str
    token_budget: int
    depth: RequestDepth
    include_evidence: bool
    scope_hint: str | None = None
    latency_budget: int | None = None
    freshness_requirement: FreshnessRequirement | None = None


@dataclass(slots=True)
class BriefingItem:
    statement: str
    confidence: float
    freshness: Freshness
    evidence_ids: list[str]


@dataclass(slots=True)
class ImpactSummary:
    reference: str
    summary: str
    changed_scope: list[str]
    stale_concepts: list[str]
    freshness_impact: FreshnessImpact


@dataclass(slots=True)
class SnapshotBinding:
    snapshot_id: str
    repo_root: str
    revision: str | None = None


@dataclass(slots=True)
class OverlayScope:
    kind: OverlayKind
    claim_scope: OverlayClaimScope
    touched_paths: list[str]
    overlay_hash: str | None = None


@dataclass(slots=True)
class VerificationPlan:
    required_checks: list[str]
    recommended_checks: list[str]
    invariants: list[str]
    coverage_gaps: list[str]
    stop_conditions: list[str]


@dataclass(slots=True)
class CoverageSlotAudit:
    slot: CoverageSlot
    status: CoverageStatus
    detail: str


@dataclass(slots=True)
class CoverageAudit:
    query_classification: QueryClassification
    required_slots: list[CoverageSlot]
    slot_results: list[CoverageSlotAudit]
    sufficient: bool
    summary: str


@dataclass(slots=True)
class BriefingPack:
    task_type: TaskType
    query_classification: QueryClassification
    consumer_type: str
    model_profile: ModelProfile
    readiness_state: ReadinessState
    snapshot_binding: SnapshotBinding
    overlay_scope: OverlayScope
    scaffolding_level: ScaffoldingLevel
    must_know: list[BriefingItem]
    relevant_flows: list[str]
    relevant_decisions: list[str]
    fragile_zones: list[str]
    do_not_break: list[str]
    suggested_files: list[str]
    reasoning_scaffold: list[str]
    impact_summary: ImpactSummary | None
    verification_plan: VerificationPlan
    coverage_audit: CoverageAudit
    verification_targets: list[str]
    evidence_index: list[EvidenceReceipt]
