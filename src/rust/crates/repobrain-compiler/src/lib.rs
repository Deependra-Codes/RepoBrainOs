use repobrain_domain::{
    BriefingItem, BriefingPack, ContextRequest, CoverageAudit, EvidenceReceipt, ImpactSummary,
    ModelClass, OverlayScope, QueryClassification, ReadinessState, ScaffoldingLevel,
    SnapshotBinding, VerificationPlan, scaffolding_for,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationPolicy {
    pub max_brief_items: usize,
}

impl CompilationPolicy {
    pub const UNBOUNDED_BRIEF_ITEMS: usize = usize::MAX;
}

impl Default for CompilationPolicy {
    fn default() -> Self {
        Self {
            max_brief_items: Self::UNBOUNDED_BRIEF_ITEMS,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompilationInput {
    pub request: ContextRequest,
    pub query_classification: QueryClassification,
    pub readiness_state: ReadinessState,
    pub snapshot_binding: SnapshotBinding,
    pub overlay_scope: OverlayScope,
    pub must_know: Vec<BriefingItem>,
    pub relevant_flows: Vec<String>,
    pub relevant_decisions: Vec<String>,
    pub fragile_zones: Vec<String>,
    pub do_not_break: Vec<String>,
    pub suggested_files: Vec<String>,
    pub impact_summary: Option<ImpactSummary>,
    pub verification_plan: VerificationPlan,
    pub coverage_audit: CoverageAudit,
    pub evidence_index: Vec<EvidenceReceipt>,
}

pub trait ContextCompiler {
    fn compile(&self, input: CompilationInput) -> BriefingPack;
}

pub struct DefaultCompiler {
    policy: CompilationPolicy,
}

impl DefaultCompiler {
    #[must_use]
    pub fn new(policy: CompilationPolicy) -> Self {
        Self { policy }
    }

    fn reasoning_scaffold(model_class: ModelClass, coverage_audit: &CoverageAudit) -> Vec<String> {
        let mut scaffold = match scaffolding_for(model_class) {
            ScaffoldingLevel::High => vec![
                "narrow scope to the smallest relevant files".to_string(),
                "validate invariants before making edits".to_string(),
                "finish with explicit verification targets".to_string(),
            ],
            ScaffoldingLevel::Medium => vec![
                "review the main flow before editing".to_string(),
                "confirm likely blast radius".to_string(),
            ],
            ScaffoldingLevel::Minimal => {
                vec!["use evidence index for drill-down when needed".to_string()]
            }
        };

        if !coverage_audit.sufficient {
            scaffold.push(format!(
                "{} Widen scope or gather more evidence before high-risk edits.",
                coverage_audit.summary
            ));
        }

        scaffold
    }

    fn verification_targets(verification_plan: &VerificationPlan) -> Vec<String> {
        let mut targets = Vec::new();

        for check in verification_plan
            .required_checks
            .iter()
            .chain(verification_plan.recommended_checks.iter())
        {
            if !targets.contains(check) {
                targets.push(check.clone());
            }
        }

        targets
    }
}

impl ContextCompiler for DefaultCompiler {
    fn compile(&self, input: CompilationInput) -> BriefingPack {
        let scaffolding_level = scaffolding_for(input.request.model_profile.class);
        let verification_targets = Self::verification_targets(&input.verification_plan);

        BriefingPack {
            task_type: input.request.task_type,
            query_classification: input.query_classification,
            consumer_type: input.request.consumer_type,
            model_profile: input.request.model_profile.clone(),
            readiness_state: input.readiness_state,
            snapshot_binding: input.snapshot_binding,
            overlay_scope: input.overlay_scope,
            scaffolding_level,
            must_know: input
                .must_know
                .into_iter()
                .take(self.policy.max_brief_items)
                .collect(),
            relevant_flows: input.relevant_flows,
            relevant_decisions: input.relevant_decisions,
            fragile_zones: input.fragile_zones,
            do_not_break: input.do_not_break,
            suggested_files: input.suggested_files,
            reasoning_scaffold: Self::reasoning_scaffold(
                input.request.model_profile.class,
                &input.coverage_audit,
            ),
            impact_summary: input.impact_summary,
            verification_plan: input.verification_plan,
            coverage_audit: input.coverage_audit,
            verification_targets,
            evidence_index: input.evidence_index,
        }
    }
}

#[cfg(test)]
mod tests {
    use repobrain_domain::{
        BriefingItem, ConsumerType, ContextRequest, CoverageAudit, CoverageSlot, CoverageSlotAudit,
        CoverageStatus, Freshness, FreshnessRequirement, ModelClass, ModelProfile,
        OverlayClaimScope, OverlayKind, OverlayScope, QueryClassification, ReadinessState,
        RequestDepth, ScaffoldingLevel, SnapshotBinding, TaskType, VerificationPlan,
    };

    use super::{CompilationInput, CompilationPolicy, ContextCompiler, DefaultCompiler};

    #[test]
    fn weak_profiles_receive_high_scaffolding() {
        let compiler = DefaultCompiler::new(CompilationPolicy::default());
        let request = ContextRequest {
            goal: "safe edit".to_string(),
            task_type: TaskType::SafeEdit,
            consumer_type: ConsumerType::Cli,
            model_profile: ModelProfile {
                id: "local-small".to_string(),
                class: ModelClass::WeakLocalCoder,
                max_context_tokens: 16_000,
                preferred_scaffolding_level: ScaffoldingLevel::High,
                notes: Vec::new(),
            },
            question: "what can break?".to_string(),
            scope_hint: Some("src/auth".to_string()),
            token_budget: 4096,
            latency_budget: Some(500),
            depth: RequestDepth::Standard,
            freshness_requirement: Some(FreshnessRequirement::FreshPreferred),
            include_evidence: true,
        };

        let pack = compiler.compile(CompilationInput {
            request,
            query_classification: QueryClassification::SafeEdit,
            readiness_state: ReadinessState::Ready,
            snapshot_binding: SnapshotBinding {
                snapshot_id: "d:/RepoBrainOS::abc123".to_string(),
                repo_root: "d:/RepoBrainOS".to_string(),
                revision: Some("abc123".to_string()),
            },
            overlay_scope: OverlayScope {
                kind: OverlayKind::Worktree,
                claim_scope: OverlayClaimScope::OverlayAdjusted,
                overlay_hash: Some("overlay42".to_string()),
                touched_paths: vec!["src/auth/api.ts".to_string()],
            },
            must_know: (0_u8..12)
                .map(|index| BriefingItem {
                    statement: format!("auth flow note {index}"),
                    confidence: 0.9,
                    freshness: Freshness::Fresh,
                    evidence_ids: vec![format!("ev_{index}")],
                })
                .collect(),
            relevant_flows: Vec::new(),
            relevant_decisions: Vec::new(),
            fragile_zones: Vec::new(),
            do_not_break: Vec::new(),
            suggested_files: Vec::new(),
            impact_summary: None,
            verification_plan: VerificationPlan {
                required_checks: vec!["logout integration test".to_string()],
                recommended_checks: vec!["legacy cookie parsing test".to_string()],
                invariants: vec!["legacy mobile tokens remain valid".to_string()],
                coverage_gaps: Vec::new(),
                stop_conditions: Vec::new(),
            },
            coverage_audit: CoverageAudit {
                query_classification: QueryClassification::SafeEdit,
                required_slots: vec![
                    CoverageSlot::ExactAnchor,
                    CoverageSlot::StructuralContext,
                    CoverageSlot::VerificationTargets,
                ],
                slot_results: vec![CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "exact anchor present".to_string(),
                }],
                sufficient: true,
                summary: "Coverage audit for `safe_edit` is sufficient.".to_string(),
            },
            evidence_index: Vec::new(),
        });

        assert_eq!(pack.scaffolding_level, ScaffoldingLevel::High);
        assert_eq!(pack.readiness_state, ReadinessState::Ready);
        assert_eq!(pack.query_classification, QueryClassification::SafeEdit);
        assert_eq!(pack.must_know.len(), 12);
        assert_eq!(
            pack.verification_targets,
            vec![
                "logout integration test".to_string(),
                "legacy cookie parsing test".to_string()
            ]
        );
    }
}
