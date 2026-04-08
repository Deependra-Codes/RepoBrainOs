use serde::{Deserialize, Serialize};

mod repo_paths;

pub use repo_paths::{canonicalize_repo_root, normalize_repo_root};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelClass {
    WeakLocalCoder,
    MediumCloudCoder,
    FrontierAgent,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaffoldingLevel {
    Minimal,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestDepth {
    Compact,
    Standard,
    Deep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Freshness {
    Fresh,
    Stale,
    Contradicted,
    Inferred,
    Unverified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessRequirement {
    Any,
    FreshPreferred,
    FreshOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LatencyClass {
    Instant,
    Interactive,
    Deep,
    Background,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    RepoOnboarding,
    SafeEdit,
    ExplainFlow,
    BlastRadius,
    SemanticDiff,
    Query,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsumerType {
    #[serde(rename = "cli")]
    Cli,
    #[serde(rename = "mcp_agent")]
    McpAgent,
    #[serde(rename = "background_worker", alias = "background_job")]
    BackgroundWorker,
    #[serde(rename = "xtask_perf", alias = "xtask-perf")]
    XtaskPerf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryClassification {
    EntityLookup,
    ArchitectureExplanation,
    SafeEdit,
    BugFix,
    FeatureImplementation,
    DecisionWhy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageSlot {
    ExactAnchor,
    StructuralContext,
    FlowSummary,
    VerificationTargets,
    ImpactEnvelope,
    DecisionEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Present,
    MissingRetrievable,
    NotObservable,
    NotApplicable,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessImpact {
    None,
    Localized,
    Moderate,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadinessState {
    Cold,
    Warming,
    Ready,
    Stale,
    Degraded,
    BulkRefresh,
    OverlayOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayKind {
    None,
    Worktree,
    Buffer,
    Mixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayClaimScope {
    SnapshotConfirmed,
    OverlayAdjusted,
    OverlayLocalOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceStage {
    L0StructuralDelta,
    L1BoundedFormal,
    L2RelationalSemantic,
    L3TheoremContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceEvidenceStatus {
    ObservedDifference,
    NoDifferenceObserved,
    ReferenceSnapshotMissing,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceContract {
    pub observable_outputs: Vec<String>,
    pub error_behavior: Vec<String>,
    pub side_effects: Vec<String>,
    pub preconditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivalenceEvidenceReceipt {
    pub backend: String,
    pub stage: EquivalenceStage,
    pub status: EquivalenceEvidenceStatus,
    pub solver: Option<String>,
    pub bounds: Option<String>,
    pub timeout_ms: Option<u64>,
    pub assumptions: Vec<String>,
    pub witness: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TheoremContract {
    pub contract_id: String,
    pub observable_outputs: Vec<String>,
    pub error_behavior: Vec<String>,
    pub side_effects: Vec<String>,
    pub preconditions: Vec<String>,
    #[serde(default)]
    pub environment_assumptions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TheoremProofObligation {
    pub id: String,
    pub source_pair: String,
    pub contract_id: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    pub encoding_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TheoremObligationStatus {
    Proved,
    Refuted,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TheoremProofCertificate {
    pub obligation_id: String,
    #[serde(default)]
    pub encoding_hash: Option<String>,
    pub status: TheoremObligationStatus,
    pub solver: Option<String>,
    pub timeout_ms: Option<u64>,
    pub assumptions: Vec<String>,
    pub witness: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TheoremRunStatus {
    ProvedUnderContract,
    RefutedUnderContract,
    InconclusiveUnderContract,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub id: String,
    pub class: ModelClass,
    pub max_context_tokens: u32,
    pub preferred_scaffolding_level: ScaffoldingLevel,
    #[serde(default)]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceReceipt {
    pub id: String,
    pub source_type: String,
    pub source_ref: String,
    pub locator: String,
    pub snippet_hash: Option<String>,
    pub captured_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextRequest {
    pub goal: String,
    pub task_type: TaskType,
    pub consumer_type: ConsumerType,
    pub model_profile: ModelProfile,
    pub question: String,
    pub scope_hint: Option<String>,
    pub token_budget: u32,
    pub latency_budget: Option<u32>,
    pub depth: RequestDepth,
    pub freshness_requirement: Option<FreshnessRequirement>,
    pub include_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefingItem {
    pub statement: String,
    pub confidence: f32,
    pub freshness: Freshness,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactSummary {
    pub reference: String,
    pub summary: String,
    pub changed_scope: Vec<String>,
    pub stale_concepts: Vec<String>,
    pub freshness_impact: FreshnessImpact,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotBinding {
    pub snapshot_id: String,
    pub repo_root: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayScope {
    pub kind: OverlayKind,
    pub claim_scope: OverlayClaimScope,
    pub overlay_hash: Option<String>,
    pub touched_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationPlan {
    pub required_checks: Vec<String>,
    pub recommended_checks: Vec<String>,
    pub invariants: Vec<String>,
    pub coverage_gaps: Vec<String>,
    pub stop_conditions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSlotAudit {
    pub slot: CoverageSlot,
    pub status: CoverageStatus,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageAudit {
    pub query_classification: QueryClassification,
    pub required_slots: Vec<CoverageSlot>,
    pub slot_results: Vec<CoverageSlotAudit>,
    pub sufficient: bool,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BriefingPack {
    pub task_type: TaskType,
    pub query_classification: QueryClassification,
    pub consumer_type: ConsumerType,
    pub model_profile: ModelProfile,
    pub readiness_state: ReadinessState,
    pub snapshot_binding: SnapshotBinding,
    pub overlay_scope: OverlayScope,
    pub scaffolding_level: ScaffoldingLevel,
    pub must_know: Vec<BriefingItem>,
    pub relevant_flows: Vec<String>,
    pub relevant_decisions: Vec<String>,
    pub fragile_zones: Vec<String>,
    pub do_not_break: Vec<String>,
    pub suggested_files: Vec<String>,
    pub reasoning_scaffold: Vec<String>,
    pub impact_summary: Option<ImpactSummary>,
    pub verification_plan: VerificationPlan,
    pub coverage_audit: CoverageAudit,
    pub verification_targets: Vec<String>,
    pub evidence_index: Vec<EvidenceReceipt>,
}

#[must_use]
pub fn scaffolding_for(model_class: ModelClass) -> ScaffoldingLevel {
    match model_class {
        ModelClass::WeakLocalCoder => ScaffoldingLevel::High,
        ModelClass::MediumCloudCoder => ScaffoldingLevel::Medium,
        ModelClass::FrontierAgent | ModelClass::Custom => ScaffoldingLevel::Minimal,
    }
}

#[cfg(test)]
mod tests {
    use super::{ModelClass, ScaffoldingLevel, scaffolding_for};

    #[test]
    fn weak_models_get_high_scaffolding() {
        assert_eq!(
            scaffolding_for(ModelClass::WeakLocalCoder),
            ScaffoldingLevel::High
        );
    }
}
