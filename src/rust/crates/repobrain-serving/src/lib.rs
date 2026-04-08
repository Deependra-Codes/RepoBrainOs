use std::path::Path;

use repobrain_domain::{
    ConsumerType, ContextRequest, FreshnessRequirement, LatencyClass, ModelClass,
    OverlayClaimScope, OverlayKind, OverlayScope, ReadinessState, RequestDepth, SnapshotBinding,
    TaskType, VerificationPlan,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingStage {
    RequestNormalization,
    ExactLookup,
    LexicalRetrieval,
    StructuralExpansion,
    SemanticRetrieval,
    CoverageAudit,
    TargetedSecondPass,
    EvidenceAssembly,
    BudgetedPacking,
    BackgroundRefresh,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotKey {
    pub repo_root: String,
    pub revision: Option<String>,
    pub overlay_hash: Option<String>,
}

impl SnapshotKey {
    #[must_use]
    pub fn new(
        repo_root: impl Into<String>,
        revision: Option<String>,
        overlay_hash: Option<String>,
    ) -> Self {
        Self {
            repo_root: repo_root.into(),
            revision,
            overlay_hash,
        }
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        format!(
            "{}::{}::{}",
            self.repo_root,
            self.revision.as_deref().unwrap_or("worktree"),
            self.overlay_hash.as_deref().unwrap_or("base")
        )
    }

    #[must_use]
    pub fn snapshot_id(&self) -> String {
        format!(
            "{}::{}",
            self.repo_root,
            self.revision.as_deref().unwrap_or("worktree")
        )
    }

    #[must_use]
    pub fn binding(&self) -> SnapshotBinding {
        SnapshotBinding {
            snapshot_id: self.snapshot_id(),
            repo_root: self.repo_root.clone(),
            revision: self.revision.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadinessAssessment {
    pub snapshot_status: SnapshotStatus,
    pub freshness_status: FreshnessStatus,
    pub serving_health: ServingHealth,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotStatus {
    Missing,
    Warming,
    Available,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FreshnessStatus {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServingHealth {
    Normal,
    Degraded,
    BulkRefresh,
    OverlayLocalOnly,
}

impl ReadinessAssessment {
    #[must_use]
    pub fn state(&self) -> ReadinessState {
        match self.snapshot_status {
            SnapshotStatus::Missing => ReadinessState::Cold,
            SnapshotStatus::Warming => ReadinessState::Warming,
            SnapshotStatus::Available => match self.serving_health {
                ServingHealth::OverlayLocalOnly => ReadinessState::OverlayOnly,
                ServingHealth::BulkRefresh => ReadinessState::BulkRefresh,
                ServingHealth::Degraded => ReadinessState::Degraded,
                ServingHealth::Normal => match self.freshness_status {
                    FreshnessStatus::Current => ReadinessState::Ready,
                    FreshnessStatus::Stale => ReadinessState::Stale,
                },
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServingMetadata {
    pub readiness_state: ReadinessState,
    pub snapshot_binding: SnapshotBinding,
    pub overlay_scope: OverlayScope,
    pub verification_plan: VerificationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServingMetadataInput {
    pub readiness_assessment: ReadinessAssessment,
    pub overlay_kind: OverlayKind,
    pub claim_scope: OverlayClaimScope,
    pub overlay_hash: Option<String>,
    pub touched_paths: Vec<String>,
    pub required_checks: Vec<String>,
    pub recommended_checks: Vec<String>,
    pub invariants: Vec<String>,
    pub coverage_gaps: Vec<String>,
    pub stop_conditions: Vec<String>,
}

#[must_use]
pub fn build_serving_metadata(
    snapshot: &SnapshotKey,
    input: ServingMetadataInput,
) -> ServingMetadata {
    ServingMetadata {
        readiness_state: input.readiness_assessment.state(),
        snapshot_binding: snapshot.binding(),
        overlay_scope: OverlayScope {
            kind: input.overlay_kind,
            claim_scope: input.claim_scope,
            overlay_hash: input.overlay_hash,
            touched_paths: input.touched_paths,
        },
        verification_plan: VerificationPlan {
            required_checks: input.required_checks,
            recommended_checks: input.recommended_checks,
            invariants: input.invariants,
            coverage_gaps: input.coverage_gaps,
            stop_conditions: input.stop_conditions,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCacheKey {
    pub normalized_query: String,
    pub task_type: TaskType,
    pub scope_hash: String,
    pub snapshot_fingerprint: String,
    pub latency_class: LatencyClass,
}

impl RetrievalCacheKey {
    #[must_use]
    pub fn from_request(
        request: &ContextRequest,
        normalized_query: impl Into<String>,
        scope_hash: impl Into<String>,
        snapshot: &SnapshotKey,
        latency_class: LatencyClass,
    ) -> Self {
        Self {
            normalized_query: normalized_query.into(),
            task_type: request.task_type,
            scope_hash: scope_hash.into(),
            snapshot_fingerprint: snapshot.fingerprint(),
            latency_class,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BriefingCacheKey {
    pub request_fingerprint: String,
    pub model_profile_bucket: String,
    pub token_budget_bucket: u32,
    pub snapshot_fingerprint: String,
}

impl BriefingCacheKey {
    #[must_use]
    pub fn from_request(request: &ContextRequest, snapshot: &SnapshotKey) -> Self {
        let scope = request.scope_hint.as_deref().unwrap_or("global");
        let request_fingerprint = format!(
            "{}::{}::{}::{}",
            task_label(request.task_type),
            request.goal,
            request.question,
            scope
        );

        Self {
            request_fingerprint,
            model_profile_bucket: format!(
                "{}-{}k",
                model_class_label(request.model_profile.class),
                request.model_profile.max_context_tokens / 1_000
            ),
            token_budget_bucket: bucket_token_budget(request.token_budget),
            snapshot_fingerprint: snapshot.fingerprint(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutingDecision {
    pub latency_class: LatencyClass,
    pub effective_freshness_requirement: FreshnessRequirement,
    pub max_candidates: usize,
    pub max_graph_hops: u8,
    pub allow_semantic_retrieval: bool,
    pub allow_second_pass: bool,
    pub allow_partial_response: bool,
    pub allowed_stages: Vec<ServingStage>,
}

impl RoutingDecision {
    #[must_use]
    pub fn allows_stage(&self, stage: ServingStage) -> bool {
        self.allowed_stages.contains(&stage)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HotPathPolicy {
    pub instant_budget_ms: u32,
    pub interactive_budget_ms: u32,
    pub deep_budget_ms: u32,
}

impl Default for HotPathPolicy {
    fn default() -> Self {
        Self {
            instant_budget_ms: 150,
            interactive_budget_ms: 800,
            deep_budget_ms: 2_500,
        }
    }
}

impl HotPathPolicy {
    #[must_use]
    pub fn classify_latency(&self, request: &ContextRequest) -> LatencyClass {
        if request.consumer_type == ConsumerType::BackgroundWorker {
            return LatencyClass::Background;
        }

        if let Some(latency_budget) = request.latency_budget {
            return self.classify_budget(latency_budget);
        }

        match (request.task_type, request.depth) {
            (TaskType::Query, RequestDepth::Compact) => LatencyClass::Instant,
            (TaskType::RepoOnboarding | TaskType::SemanticDiff, _) | (_, RequestDepth::Deep) => {
                LatencyClass::Deep
            }
            _ => LatencyClass::Interactive,
        }
    }

    #[must_use]
    pub fn route(&self, request: &ContextRequest) -> RoutingDecision {
        let latency_class = self.classify_latency(request);
        let effective_freshness_requirement =
            request
                .freshness_requirement
                .unwrap_or(match request.task_type {
                    TaskType::SafeEdit | TaskType::BlastRadius | TaskType::SemanticDiff => {
                        FreshnessRequirement::FreshPreferred
                    }
                    _ => FreshnessRequirement::Any,
                });
        let allow_semantic_retrieval =
            latency_class == LatencyClass::Deep && request.depth == RequestDepth::Deep;
        let allow_second_pass = match latency_class {
            LatencyClass::Instant | LatencyClass::Background => false,
            LatencyClass::Interactive => {
                request.task_type != TaskType::Query && request.depth != RequestDepth::Compact
            }
            LatencyClass::Deep => true,
        };

        RoutingDecision {
            latency_class,
            effective_freshness_requirement,
            max_candidates: candidate_budget(request.task_type, latency_class),
            max_graph_hops: graph_hop_budget(latency_class),
            allow_semantic_retrieval,
            allow_second_pass,
            allow_partial_response: latency_class != LatencyClass::Background,
            allowed_stages: allowed_stages(
                latency_class,
                allow_semantic_retrieval,
                allow_second_pass,
            ),
        }
    }

    #[must_use]
    fn classify_budget(&self, latency_budget: u32) -> LatencyClass {
        if latency_budget <= self.instant_budget_ms {
            LatencyClass::Instant
        } else if latency_budget <= self.interactive_budget_ms {
            LatencyClass::Interactive
        } else {
            LatencyClass::Deep
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    pub snapshot: SnapshotKey,
    pub changed_files: Vec<String>,
    pub changed_symbols: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshPriority {
    Immediate,
    Soon,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundRefreshJobKind {
    UpdateExactIndex,
    UpdateLexicalIndex,
    UpdateStructuralIndex,
    MarkStaleRecords,
    RegenerateFlowCapsules,
    RegenerateArchitectureCapsules,
    RegenerateDecisionRecords,
    BuildEmbeddings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundRefreshJob {
    pub snapshot_fingerprint: String,
    pub kind: BackgroundRefreshJobKind,
    pub priority: RefreshPriority,
}

pub struct BackgroundRefreshPlanner;

impl BackgroundRefreshPlanner {
    #[must_use]
    pub fn plan(change_set: &ChangeSet) -> Vec<BackgroundRefreshJob> {
        let snapshot_fingerprint = change_set.snapshot.fingerprint();
        let mut jobs = vec![
            BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::UpdateExactIndex,
                priority: RefreshPriority::Immediate,
            },
            BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::UpdateLexicalIndex,
                priority: RefreshPriority::Immediate,
            },
            BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::UpdateStructuralIndex,
                priority: RefreshPriority::Immediate,
            },
            BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::MarkStaleRecords,
                priority: RefreshPriority::Immediate,
            },
        ];

        if has_non_markdown_changes(&change_set.changed_files) {
            jobs.push(BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::RegenerateFlowCapsules,
                priority: RefreshPriority::Soon,
            });
            jobs.push(BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::RegenerateArchitectureCapsules,
                priority: RefreshPriority::Soon,
            });
        }

        if !change_set.changed_symbols.is_empty() || has_markdown_changes(&change_set.changed_files)
        {
            jobs.push(BackgroundRefreshJob {
                snapshot_fingerprint: snapshot_fingerprint.clone(),
                kind: BackgroundRefreshJobKind::RegenerateDecisionRecords,
                priority: RefreshPriority::Deferred,
            });
        }

        if !change_set.changed_symbols.is_empty() {
            jobs.push(BackgroundRefreshJob {
                snapshot_fingerprint,
                kind: BackgroundRefreshJobKind::BuildEmbeddings,
                priority: RefreshPriority::Deferred,
            });
        }

        jobs
    }
}

#[must_use]
fn candidate_budget(task_type: TaskType, latency_class: LatencyClass) -> usize {
    match latency_class {
        LatencyClass::Instant => match task_type {
            TaskType::Query => 8,
            _ => 10,
        },
        LatencyClass::Interactive => match task_type {
            TaskType::SafeEdit | TaskType::BlastRadius => 24,
            TaskType::ExplainFlow => 20,
            _ => 16,
        },
        LatencyClass::Deep => match task_type {
            TaskType::RepoOnboarding | TaskType::SemanticDiff => 40,
            TaskType::SafeEdit | TaskType::BlastRadius | TaskType::ExplainFlow => 32,
            TaskType::Query => 20,
        },
        LatencyClass::Background => 0,
    }
}

#[must_use]
fn graph_hop_budget(latency_class: LatencyClass) -> u8 {
    match latency_class {
        LatencyClass::Instant => 1,
        LatencyClass::Interactive => 2,
        LatencyClass::Deep => 3,
        LatencyClass::Background => 0,
    }
}

#[must_use]
fn allowed_stages(
    latency_class: LatencyClass,
    allow_semantic_retrieval: bool,
    allow_second_pass: bool,
) -> Vec<ServingStage> {
    match latency_class {
        LatencyClass::Background => vec![ServingStage::BackgroundRefresh],
        LatencyClass::Instant => vec![
            ServingStage::RequestNormalization,
            ServingStage::ExactLookup,
            ServingStage::LexicalRetrieval,
            ServingStage::StructuralExpansion,
            ServingStage::BudgetedPacking,
        ],
        LatencyClass::Interactive | LatencyClass::Deep => {
            let mut stages = vec![
                ServingStage::RequestNormalization,
                ServingStage::ExactLookup,
                ServingStage::LexicalRetrieval,
                ServingStage::StructuralExpansion,
                ServingStage::CoverageAudit,
                ServingStage::EvidenceAssembly,
                ServingStage::BudgetedPacking,
            ];

            if allow_semantic_retrieval {
                stages.insert(4, ServingStage::SemanticRetrieval);
            }

            if allow_second_pass {
                stages.insert(stages.len() - 2, ServingStage::TargetedSecondPass);
            }

            stages
        }
    }
}

#[must_use]
fn has_markdown_changes(changed_files: &[String]) -> bool {
    changed_files.iter().any(|path| is_markdown_path(path))
}

#[must_use]
fn has_non_markdown_changes(changed_files: &[String]) -> bool {
    changed_files.iter().any(|path| !is_markdown_path(path))
}

#[must_use]
fn is_markdown_path(path: &str) -> bool {
    Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
}

#[must_use]
fn bucket_token_budget(token_budget: u32) -> u32 {
    token_budget.div_ceil(1_024) * 1_024
}

#[must_use]
fn task_label(task_type: TaskType) -> &'static str {
    match task_type {
        TaskType::RepoOnboarding => "repo_onboarding",
        TaskType::SafeEdit => "safe_edit",
        TaskType::ExplainFlow => "explain_flow",
        TaskType::BlastRadius => "blast_radius",
        TaskType::SemanticDiff => "semantic_diff",
        TaskType::Query => "query",
    }
}

#[must_use]
fn model_class_label(model_class: ModelClass) -> &'static str {
    match model_class {
        ModelClass::WeakLocalCoder => "weak_local_coder",
        ModelClass::MediumCloudCoder => "medium_cloud_coder",
        ModelClass::FrontierAgent => "frontier_agent",
        ModelClass::Custom => "custom",
    }
}

#[cfg(test)]
mod tests {
    use repobrain_domain::{
        ConsumerType, FreshnessRequirement, LatencyClass, ModelClass, ModelProfile,
        OverlayClaimScope, OverlayKind, ReadinessState, RequestDepth, ScaffoldingLevel, TaskType,
    };

    use super::{
        BackgroundRefreshJobKind, BackgroundRefreshPlanner, BriefingCacheKey, ChangeSet,
        FreshnessStatus, HotPathPolicy, ReadinessAssessment, RetrievalCacheKey, ServingHealth,
        ServingMetadataInput, ServingStage, SnapshotKey, SnapshotStatus, build_serving_metadata,
    };

    fn request(
        task_type: TaskType,
        depth: RequestDepth,
        latency_budget: Option<u32>,
    ) -> repobrain_domain::ContextRequest {
        repobrain_domain::ContextRequest {
            goal: "answer a repo question".to_string(),
            task_type,
            consumer_type: ConsumerType::Cli,
            model_profile: ModelProfile {
                id: "frontier".to_string(),
                class: ModelClass::FrontierAgent,
                max_context_tokens: 128_000,
                preferred_scaffolding_level: ScaffoldingLevel::Minimal,
                notes: Vec::new(),
            },
            question: "what should I know?".to_string(),
            scope_hint: Some("src/auth".to_string()),
            token_budget: 1_750,
            latency_budget,
            depth,
            freshness_requirement: Some(FreshnessRequirement::FreshPreferred),
            include_evidence: true,
        }
    }

    #[test]
    fn compact_query_defaults_to_instant() {
        let policy = HotPathPolicy::default();
        let decision = policy.route(&request(TaskType::Query, RequestDepth::Compact, None));

        assert_eq!(decision.latency_class, LatencyClass::Instant);
        assert!(decision.allows_stage(ServingStage::ExactLookup));
        assert!(!decision.allows_stage(ServingStage::SemanticRetrieval));
    }

    #[test]
    fn safe_edit_budget_routes_to_interactive() {
        let policy = HotPathPolicy::default();
        let decision = policy.route(&request(
            TaskType::SafeEdit,
            RequestDepth::Standard,
            Some(500),
        ));

        assert_eq!(decision.latency_class, LatencyClass::Interactive);
        assert!(decision.allow_second_pass);
        assert_eq!(decision.max_graph_hops, 2);
    }

    #[test]
    fn deep_requests_can_enable_semantic_retrieval() {
        let policy = HotPathPolicy::default();
        let decision = policy.route(&request(
            TaskType::ExplainFlow,
            RequestDepth::Deep,
            Some(1_500),
        ));

        assert_eq!(decision.latency_class, LatencyClass::Deep);
        assert!(decision.allow_semantic_retrieval);
        assert!(decision.allows_stage(ServingStage::SemanticRetrieval));
    }

    #[test]
    fn background_workers_route_to_background() {
        let policy = HotPathPolicy::default();
        let mut background_request = request(TaskType::SemanticDiff, RequestDepth::Deep, None);
        background_request.consumer_type = ConsumerType::BackgroundWorker;
        let decision = policy.route(&background_request);

        assert_eq!(decision.latency_class, LatencyClass::Background);
        assert_eq!(
            decision.allowed_stages,
            vec![ServingStage::BackgroundRefresh]
        );
    }

    #[test]
    fn cache_keys_use_snapshot_fingerprints_and_buckets() {
        let snapshot = SnapshotKey::new(
            "d:/RepoBrainOS",
            Some("abc123".to_string()),
            Some("overlay42".to_string()),
        );
        let request = request(TaskType::SafeEdit, RequestDepth::Standard, Some(500));
        let retrieval = RetrievalCacheKey::from_request(
            &request,
            "auth flow",
            "scope123",
            &snapshot,
            LatencyClass::Interactive,
        );
        let briefing = BriefingCacheKey::from_request(&request, &snapshot);

        assert_eq!(
            retrieval.snapshot_fingerprint,
            "d:/RepoBrainOS::abc123::overlay42"
        );
        assert_eq!(briefing.token_budget_bucket, 2_048);
        assert!(briefing.model_profile_bucket.starts_with("frontier_agent"));
    }

    #[test]
    fn planner_emits_immediate_and_deferred_jobs() {
        let change_set = ChangeSet {
            snapshot: SnapshotKey::new("d:/RepoBrainOS", Some("abc123".to_string()), None),
            changed_files: vec!["src/auth/api.ts".to_string(), "docs/auth.md".to_string()],
            changed_symbols: vec!["AuthSession".to_string()],
        };

        let jobs = BackgroundRefreshPlanner::plan(&change_set);

        assert!(
            jobs.iter()
                .any(|job| job.kind == BackgroundRefreshJobKind::UpdateExactIndex)
        );
        assert!(
            jobs.iter()
                .any(|job| job.kind == BackgroundRefreshJobKind::RegenerateFlowCapsules)
        );
        assert!(
            jobs.iter()
                .any(|job| job.kind == BackgroundRefreshJobKind::RegenerateDecisionRecords)
        );
        assert!(
            jobs.iter()
                .any(|job| job.kind == BackgroundRefreshJobKind::BuildEmbeddings)
        );
    }

    #[test]
    fn readiness_assessment_prefers_overlay_only_when_local_overlay_is_authoritative() {
        let assessment = ReadinessAssessment {
            snapshot_status: SnapshotStatus::Available,
            freshness_status: FreshnessStatus::Current,
            serving_health: ServingHealth::OverlayLocalOnly,
        };

        assert_eq!(assessment.state(), ReadinessState::OverlayOnly);
    }

    #[test]
    fn serving_metadata_projects_snapshot_binding_overlay_and_plan() {
        let snapshot = SnapshotKey::new(
            "d:/RepoBrainOS",
            Some("abc123".to_string()),
            Some("overlay42".to_string()),
        );
        let metadata = build_serving_metadata(
            &snapshot,
            ServingMetadataInput {
                readiness_assessment: ReadinessAssessment {
                    snapshot_status: SnapshotStatus::Available,
                    freshness_status: FreshnessStatus::Stale,
                    serving_health: ServingHealth::Normal,
                },
                overlay_kind: OverlayKind::Worktree,
                claim_scope: OverlayClaimScope::OverlayAdjusted,
                overlay_hash: Some("overlay42".to_string()),
                touched_paths: vec!["src/auth/api.ts".to_string()],
                required_checks: vec!["logout integration test".to_string()],
                recommended_checks: vec!["legacy cookie parsing test".to_string()],
                invariants: vec!["legacy mobile tokens remain valid".to_string()],
                coverage_gaps: vec!["no session-store property test".to_string()],
                stop_conditions: vec!["stop if invalidation path is unclear".to_string()],
            },
        );

        assert_eq!(metadata.readiness_state, ReadinessState::Stale);
        assert_eq!(
            metadata.snapshot_binding.snapshot_id,
            "d:/RepoBrainOS::abc123"
        );
        assert_eq!(metadata.overlay_scope.kind, OverlayKind::Worktree);
        assert_eq!(
            metadata.verification_plan.required_checks,
            vec!["logout integration test".to_string()]
        );
    }
}
