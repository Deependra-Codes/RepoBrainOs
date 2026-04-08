export type ModelClass = "weak_local_coder" | "medium_cloud_coder" | "frontier_agent" | "custom";

export type ScaffoldingLevel = "minimal" | "medium" | "high";
export type RequestDepth = "compact" | "standard" | "deep";
export type FreshnessRequirement = "any" | "fresh_preferred" | "fresh_only";
export type Freshness = "fresh" | "stale" | "contradicted" | "inferred" | "unverified";
export type LatencyClass = "instant" | "interactive" | "deep" | "background";
export type ReadinessState =
  | "cold"
  | "warming"
  | "ready"
  | "stale"
  | "degraded"
  | "bulk_refresh"
  | "overlay_only";
export type TaskType =
  | "repo_onboarding"
  | "safe_edit"
  | "explain_flow"
  | "blast_radius"
  | "semantic_diff"
  | "query";
export type ConsumerType =
  | "cli"
  | "mcp_agent"
  | "background_worker"
  | "background_job"
  | "xtask_perf"
  | "xtask-perf";
export type QueryClassification =
  | "entity_lookup"
  | "architecture_explanation"
  | "safe_edit"
  | "bug_fix"
  | "feature_implementation"
  | "decision_why";
export type CoverageSlot =
  | "exact_anchor"
  | "structural_context"
  | "flow_summary"
  | "verification_targets"
  | "impact_envelope"
  | "decision_evidence";
export type CoverageStatus =
  | "present"
  | "missing_retrievable"
  | "not_observable"
  | "not_applicable"
  | "stale";
export type FreshnessImpact = "none" | "localized" | "moderate" | "high";
export type OverlayKind = "none" | "worktree" | "buffer" | "mixed";
export type OverlayClaimScope = "snapshot_confirmed" | "overlay_adjusted" | "overlay_local_only";

export interface ModelProfile {
  id: string;
  class: ModelClass;
  maxContextTokens: number;
  preferredScaffoldingLevel: ScaffoldingLevel;
  notes?: string[];
}

export interface EvidenceReceipt {
  id: string;
  sourceType: string;
  sourceRef: string;
  locator: string;
  snippetHash?: string;
  capturedAt: string;
}

export interface BriefingItem {
  statement: string;
  confidence: number;
  freshness: Freshness;
  evidenceIds: string[];
}

export interface ImpactSummary {
  reference: string;
  summary: string;
  changedScope: string[];
  staleConcepts: string[];
  freshnessImpact: FreshnessImpact;
}

export interface SnapshotBinding {
  snapshotId: string;
  repoRoot: string;
  revision?: string;
}

export interface OverlayScope {
  kind: OverlayKind;
  claimScope: OverlayClaimScope;
  overlayHash?: string;
  touchedPaths: string[];
}

export interface VerificationPlan {
  requiredChecks: string[];
  recommendedChecks: string[];
  invariants: string[];
  coverageGaps: string[];
  stopConditions: string[];
}

export interface CoverageSlotAudit {
  slot: CoverageSlot;
  status: CoverageStatus;
  detail: string;
}

export interface CoverageAudit {
  queryClassification: QueryClassification;
  requiredSlots: CoverageSlot[];
  slotResults: CoverageSlotAudit[];
  sufficient: boolean;
  summary: string;
}

export interface ContextRequest {
  goal: string;
  taskType: TaskType;
  consumerType: ConsumerType;
  modelProfile: ModelProfile;
  question: string;
  scopeHint?: string;
  tokenBudget: number;
  latencyBudget?: number;
  depth: RequestDepth;
  freshnessRequirement?: FreshnessRequirement;
  includeEvidence: boolean;
}

export interface BriefingPack {
  taskType: TaskType;
  queryClassification: QueryClassification;
  consumerType: ConsumerType;
  modelProfile: ModelProfile;
  readinessState: ReadinessState;
  snapshotBinding: SnapshotBinding;
  overlayScope: OverlayScope;
  scaffoldingLevel: ScaffoldingLevel;
  mustKnow: BriefingItem[];
  relevantFlows: string[];
  relevantDecisions: string[];
  fragileZones: string[];
  doNotBreak: string[];
  suggestedFiles: string[];
  reasoningScaffold: string[];
  impactSummary?: ImpactSummary;
  verificationPlan: VerificationPlan;
  coverageAudit: CoverageAudit;
  verificationTargets: string[];
  evidenceIndex: EvidenceReceipt[];
}

export interface ToolDefinition {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}
