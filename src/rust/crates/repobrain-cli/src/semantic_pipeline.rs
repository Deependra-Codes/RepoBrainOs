use super::{
    BTreeMap, BTreeSet, CliL1KaniMode, CliL2AlignmentMode, CliSemanticDiffStage,
    CliTheoremSolverMode, CliTranslationIrMode, CliTranslationRunStatusMode, ConsumerType,
    ContextRequest, Duration, EquivalenceContract, EquivalenceEvidenceReceipt,
    EquivalenceEvidenceStatus, EquivalenceStage, ErrorKind, ExplainFlowArgs, ExplainFlowOutput,
    FreshnessRequirement, GraphStore, IngestError, Instant, ListInvariantsArgs,
    ListInvariantsOutput, Output, Path, PathBuf, ProcessCommand, ReplayTheoremArgs,
    RepositoryInventorySnapshot, RequestDepth, Result, SnapshotArtifactStore, SnapshotBrokerInput,
    SnapshotContextBroker, SnapshotGraphStore, Stdio, SymbolKind, TaskType, TheoremContract,
    TheoremObligationStatus, TheoremProofCertificate, TheoremProofObligation, TheoremRunStatus,
    WhatChangedSemanticallyArgs, bail, cli_flow_depth_label, default_model_profile, flow_trace, fs,
    merged_invariants, readiness_label, resolve_repo_root, serving_metadata, serving_runtime_input,
    symbol_evidence_label, symbol_kind_label,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

mod ivl;
mod l1;
mod l2;
mod theorem;

pub(super) use l2::{l2_relational_semantic_receipt, l2_status_label};
pub(super) use theorem::theorem_stage_artifacts;

use l1::{l1_rust_bounded_receipt, l1_summary};
use l2::{l2_summary, semantic_delta_witness};
use theorem::theorem_run_status_label;
use theorem::{
    theorem_stage_missing_contract_receipt, theorem_stage_receipt, theorem_stage_summary,
};

#[cfg(test)]
pub(super) use theorem::{
    theorem_obligation_for_source_pair, theorem_schedule_obligations,
    theorem_translation_validation_receipt,
};

#[cfg(test)]
pub(super) use l1::{
    is_rust_source_path, l1_collect_proof_obligations, l1_rust_bounded_scope, l1_status_label,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SemanticDiffOutput {
    snapshot_id: String,
    readiness_state: String,
    reference: String,
    reference_snapshot_id: Option<String>,
    status: String,
    summary: String,
    equivalence_stage: EquivalenceStage,
    equivalence_contract: EquivalenceContract,
    evidence_receipts: Vec<EquivalenceEvidenceReceipt>,
    theorem_contract: Option<TheoremContract>,
    theorem_run_status: Option<TheoremRunStatus>,
    theorem_obligations: Vec<TheoremProofObligation>,
    theorem_certificates: Vec<TheoremProofCertificate>,
    theorem_replay_artifact: Option<String>,
    theorem_replay_command: Option<String>,
    planned_backlog: Vec<String>,
    changed_scope: Vec<String>,
    added_files: Vec<String>,
    removed_files: Vec<String>,
    modified_files: Vec<String>,
    added_symbol_facts: usize,
    removed_symbol_facts: usize,
    added_import_facts: usize,
    removed_import_facts: usize,
    added_verification_targets: Vec<String>,
    removed_verification_targets: Vec<String>,
    recommended_next_steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct TheoremStageArtifacts {
    pub(super) contract: Option<TheoremContract>,
    pub(super) run_status: Option<TheoremRunStatus>,
    pub(super) obligations: Vec<TheoremProofObligation>,
    pub(super) certificates: Vec<TheoremProofCertificate>,
    pub(super) replay_artifact: Option<String>,
    pub(super) replay_command: Option<String>,
    pub(super) extra_receipts: Vec<EquivalenceEvidenceReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SemanticDelta {
    pub(super) changed_scope: Vec<String>,
    pub(super) added_files: Vec<String>,
    pub(super) removed_files: Vec<String>,
    pub(super) modified_files: Vec<String>,
    pub(super) added_symbol_facts: usize,
    pub(super) removed_symbol_facts: usize,
    pub(super) added_import_facts: usize,
    pub(super) removed_import_facts: usize,
    pub(super) added_verification_targets: Vec<String>,
    pub(super) removed_verification_targets: Vec<String>,
}

impl SemanticDelta {
    pub(super) fn has_changes(&self) -> bool {
        !self.changed_scope.is_empty()
            || !self.added_files.is_empty()
            || !self.removed_files.is_empty()
            || !self.modified_files.is_empty()
            || self.added_symbol_facts != 0
            || self.removed_symbol_facts != 0
            || self.added_import_facts != 0
            || self.removed_import_facts != 0
            || !self.added_verification_targets.is_empty()
            || !self.removed_verification_targets.is_empty()
    }
}

#[derive(Debug, Clone, Copy)]
struct L1RustBoundedPolicy {
    max_rust_files: usize,
    max_functions: usize,
    timeout_ms: u64,
    kani_mode: CliL1KaniMode,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct L2RelationalPolicy {
    pub(super) max_pairs: usize,
    pub(super) max_candidate_functions: usize,
    pub(super) timeout_ms: u64,
    pub(super) alignment_mode: CliL2AlignmentMode,
    pub(super) min_alignment_score: u8,
}

#[derive(Debug, Clone)]
pub(super) struct TheoremPolicy {
    pub(super) max_obligations: usize,
    pub(super) max_candidate_functions: usize,
    pub(super) timeout_ms: u64,
    pub(super) flaky_retries: u8,
    pub(super) stability_runs: u8,
    pub(super) max_parallelism: usize,
    pub(super) solver_mode: CliTheoremSolverMode,
    pub(super) solver_path: Option<PathBuf>,
    pub(super) solver_timeout_ms: u64,
    pub(super) solver_fallback_relational: bool,
    pub(super) replay_out: Option<PathBuf>,
    pub(super) persist_replay: bool,
    pub(super) translation_validation: bool,
    pub(super) alive2_path: Option<PathBuf>,
    pub(super) translation_max_obligations: usize,
    pub(super) translation_timeout_ms: u64,
    pub(super) translation_ir_mode: CliTranslationIrMode,
    pub(super) translation_run_status_mode: CliTranslationRunStatusMode,
}

#[derive(Debug)]
pub(super) struct L1RustBoundedScope {
    pub(super) rust_files: Vec<String>,
    pub(super) function_symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct L1ProofObligation {
    pub(super) manifest_path: String,
    pub(super) source_path: String,
    pub(super) harness_name: String,
    pub(super) line_number: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L1ObligationExecution {
    observed_difference: usize,
    no_difference_observed: usize,
    inconclusive: usize,
    timed_out: bool,
    witness: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2BehaviorProfile {
    declaration_signature: String,
    call_signature: String,
    control_signature: String,
    literal_signature: String,
    normalized_window_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2FunctionSignal {
    id: String,
    name: String,
    name_key: String,
    relative_path: String,
    manifest_path: Option<String>,
    language: String,
    line_number: u32,
    import_signature: String,
    reverse_import_signature: String,
    symbol_context_signature: String,
    behavior_profile: L2BehaviorProfile,
    behavior_signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2AlignedPair {
    reference: L2FunctionSignal,
    target: L2FunctionSignal,
    strategy: &'static str,
    score: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2AlignmentResult {
    pairs: Vec<L2AlignedPair>,
    unmatched_reference: Vec<L2FunctionSignal>,
    unmatched_target: Vec<L2FunctionSignal>,
    timed_out: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct L2RelationalOutcome {
    compared_pairs: usize,
    differing_pairs: usize,
    unchanged_pairs: usize,
    unmatched_reference: usize,
    unmatched_target: usize,
    timed_out: bool,
    witness: Option<String>,
}

#[derive(Debug)]
enum CommandRunOutcome {
    Completed(Output),
    TimedOut(Option<Output>),
    SpawnFailed(String),
    IoError(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TheoremReplayArtifact {
    pub(super) artifact_version: String,
    pub(super) snapshot_id: String,
    pub(super) reference_snapshot_id: String,
    pub(super) contract_id: String,
    pub(super) policy: TheoremReplayPolicy,
    pub(super) obligations: Vec<TheoremProofObligation>,
    pub(super) certificates: Vec<TheoremProofCertificate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct TheoremReplayPolicy {
    pub(super) max_obligations: usize,
    pub(super) max_candidate_functions: usize,
    pub(super) timeout_ms: u64,
    pub(super) flaky_retries: u8,
    pub(super) stability_runs: u8,
    pub(super) max_parallelism: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReplayTheoremOutput {
    artifact: String,
    contract_id: String,
    snapshot_id: String,
    reference_snapshot_id: String,
    obligations: usize,
    certificates: usize,
    selected_obligation: Option<String>,
    replay_records: Vec<ReplayTheoremRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ReplayTheoremRecord {
    obligation_id: String,
    source_pair: String,
    encoding_hash: String,
    status: TheoremObligationStatus,
    solver: Option<String>,
    timeout_ms: Option<u64>,
    witness: Option<String>,
}

#[derive(Debug)]
struct TheoremExecutionContext {
    reference_map: BTreeMap<String, L2FunctionSignal>,
    target_map: BTreeMap<String, L2FunctionSignal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TheoremEvaluation {
    status: TheoremObligationStatus,
    solver: Option<String>,
    assumptions: Vec<String>,
    witness: Option<String>,
}

struct SemanticDiffBuildContext<'a> {
    repo_root: &'a Path,
    snapshot: &'a RepositoryInventorySnapshot,
    metadata: &'a repobrain_serving::ServingMetadata,
    reference_snapshot: &'a RepositoryInventorySnapshot,
    reference: &'a str,
    requested_stage: CliSemanticDiffStage,
    l1_policy: L1RustBoundedPolicy,
    l2_policy: L2RelationalPolicy,
    theorem_policy: TheoremPolicy,
    equivalence_contract: &'a EquivalenceContract,
    theorem_contract: Option<&'a TheoremContract>,
    max_changed_scope_items: usize,
}

pub(super) fn explain_flow(args: ExplainFlowArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let graph = SnapshotGraphStore::new(&snapshot);
    let report = graph.blast_radius(&args.flow_name);
    if report.flow_capsules.is_empty() {
        bail!(
            "flow `{}` did not produce structural flow capsules in snapshot {}",
            args.flow_name,
            metadata.snapshot_binding.snapshot_id
        );
    }

    let flow_capsules = report
        .flow_capsules
        .iter()
        .map(|capsule| flow_trace(capsule, args.depth))
        .collect::<Vec<_>>();
    let output = ExplainFlowOutput {
        snapshot_id: metadata.snapshot_binding.snapshot_id,
        readiness_state: readiness_label(metadata.readiness_state).to_string(),
        flow_name: args.flow_name,
        depth: cli_flow_depth_label(args.depth).to_string(),
        impacted_nodes: report.impacted_nodes.len(),
        relationships: report.relationships.len(),
        flow_capsules,
        verification_targets: report.verification_targets,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .context("failed to serialize explain-flow output as json")?
    );

    Ok(())
}

pub(super) fn list_invariants(args: ListInvariantsArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let broker = SnapshotContextBroker::default();
    let briefing = broker
        .get_brief(SnapshotBrokerInput {
            request: ContextRequest {
                goal: format!("list invariants for {}", args.scope),
                task_type: TaskType::SafeEdit,
                consumer_type: ConsumerType::Cli,
                model_profile: default_model_profile(),
                question: format!("what invariants should remain true for {}?", args.scope),
                scope_hint: Some(args.scope.clone()),
                token_budget: args.token_budget,
                latency_budget: Some(800),
                depth: RequestDepth::Standard,
                freshness_requirement: Some(FreshnessRequirement::FreshPreferred),
                include_evidence: true,
            },
            snapshot: &snapshot,
            readiness_assessment: serving.readiness_assessment,
            overlay_kind: serving.overlay_kind,
            claim_scope: serving.claim_scope,
            overlay_hash: serving.overlay_hash.clone(),
            touched_paths: serving.touched_paths.clone(),
        })
        .context("failed to compile snapshot-backed briefing pack for invariant listing")?;
    let invariants = merged_invariants(&briefing);
    let output = ListInvariantsOutput {
        snapshot_id: metadata.snapshot_binding.snapshot_id,
        readiness_state: readiness_label(metadata.readiness_state).to_string(),
        scope: args.scope,
        invariants,
        do_not_break: briefing.do_not_break,
        required_checks: briefing.verification_plan.required_checks,
        recommended_checks: briefing.verification_plan.recommended_checks,
        coverage_gaps: briefing.verification_plan.coverage_gaps,
        stop_conditions: briefing.verification_plan.stop_conditions,
        coverage_summary: briefing.coverage_audit.summary,
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .context("failed to serialize list-invariants output as json")?
    );

    Ok(())
}

pub(super) fn what_changed_semantically(args: WhatChangedSemanticallyArgs) -> Result<()> {
    const MAX_CHANGED_SCOPE_ITEMS: usize = 32;

    let serving = serving_runtime_input(&args.serving);
    let requested_stage = args.stage;
    let l1_policy = l1_policy_from_args(&args);
    let l2_policy = l2_policy_from_args(&args);
    let theorem_policy = theorem_policy_from_args(&args);
    let theorem_contract = load_theorem_contract_for_stage(&args, requested_stage)?;
    let repo_root = resolve_repo_root(args.repo_root)?;
    let revision = args.revision;
    let reference = args.r#ref;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, revision.clone(), &serving);
    let equivalence_contract = theorem_contract.as_ref().map_or_else(
        semantic_equivalence_contract,
        equivalence_contract_from_theorem_contract,
    );
    let reference_snapshot = load_reference_snapshot(&store, &reference)?;
    let output = if let Some(reference_snapshot) = reference_snapshot {
        let context = SemanticDiffBuildContext {
            repo_root: &repo_root,
            snapshot: &snapshot,
            metadata: &metadata,
            reference_snapshot: &reference_snapshot,
            reference: &reference,
            requested_stage,
            l1_policy,
            l2_policy,
            theorem_policy,
            equivalence_contract: &equivalence_contract,
            theorem_contract: theorem_contract.as_ref(),
            max_changed_scope_items: MAX_CHANGED_SCOPE_ITEMS,
        };
        semantic_diff_output_with_reference(&context)
    } else {
        semantic_diff_output_missing_reference(
            &repo_root,
            &metadata,
            &reference,
            requested_stage,
            equivalence_contract,
            theorem_contract,
        )
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .context("failed to serialize semantic-diff output as json")?
    );

    Ok(())
}

pub(super) fn replay_theorem(args: ReplayTheoremArgs) -> Result<()> {
    let serialized = fs::read_to_string(&args.artifact)
        .with_context(|| format!("failed to read replay artifact {}", args.artifact.display()))?;
    let artifact: TheoremReplayArtifact = serde_json::from_str(&serialized).with_context(|| {
        format!(
            "failed to parse replay artifact json {}",
            args.artifact.display()
        )
    })?;
    let obligation_filter = args.obligation_id.as_deref();
    let mut obligation_by_id = artifact
        .obligations
        .iter()
        .map(|obligation| (obligation.id.clone(), obligation))
        .collect::<BTreeMap<_, _>>();
    let mut replay_records = Vec::new();

    for certificate in &artifact.certificates {
        if let Some(obligation_id) = obligation_filter
            && certificate.obligation_id != obligation_id
        {
            continue;
        }
        let Some(obligation) = obligation_by_id.remove(&certificate.obligation_id) else {
            continue;
        };
        replay_records.push(ReplayTheoremRecord {
            obligation_id: certificate.obligation_id.clone(),
            source_pair: obligation.source_pair.clone(),
            encoding_hash: obligation.encoding_hash.clone(),
            status: certificate.status,
            solver: certificate.solver.clone(),
            timeout_ms: certificate.timeout_ms,
            witness: certificate.witness.clone(),
        });
    }

    replay_records.sort_unstable_by(|left, right| left.obligation_id.cmp(&right.obligation_id));
    let output = ReplayTheoremOutput {
        artifact: args.artifact.to_string_lossy().to_string(),
        contract_id: artifact.contract_id,
        snapshot_id: artifact.snapshot_id,
        reference_snapshot_id: artifact.reference_snapshot_id,
        obligations: artifact.obligations.len(),
        certificates: artifact.certificates.len(),
        selected_obligation: args.obligation_id,
        replay_records,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&output)
            .context("failed to serialize theorem replay output as json")?
    );

    Ok(())
}

fn load_reference_snapshot(
    store: &SnapshotArtifactStore,
    reference: &str,
) -> Result<Option<RepositoryInventorySnapshot>> {
    match store.load_inventory(Some(reference)) {
        Ok(snapshot) => Ok(Some(snapshot)),
        Err(IngestError::Io { source, .. }) if source.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(error).with_context(|| format!("failed to load reference snapshot `{reference}`"))
        }
    }
}

pub(super) fn load_theorem_contract_for_stage(
    args: &WhatChangedSemanticallyArgs,
    stage: CliSemanticDiffStage,
) -> Result<Option<TheoremContract>> {
    if !matches!(stage, CliSemanticDiffStage::L3TheoremContract) {
        return Ok(None);
    }

    let Some(contract_path) = args.theorem_contract_file.as_ref() else {
        bail!(
            "theorem stage requires --theorem-contract-file <path> with an explicit theorem contract"
        );
    };
    let serialized = fs::read_to_string(contract_path).with_context(|| {
        format!(
            "failed to read theorem contract {}",
            contract_path.display()
        )
    })?;
    let contract = serde_json::from_str::<TheoremContract>(&serialized).with_context(|| {
        format!(
            "failed to parse theorem contract json {}",
            contract_path.display()
        )
    })?;
    validate_theorem_contract(&contract)?;

    Ok(Some(contract))
}

fn validate_theorem_contract(contract: &TheoremContract) -> Result<()> {
    if contract.contract_id.trim().is_empty() {
        bail!("theorem contract must include a non-empty contract_id");
    }
    if contract.observable_outputs.is_empty() {
        bail!("theorem contract requires at least one observable output");
    }
    if contract.error_behavior.is_empty() {
        bail!("theorem contract requires at least one error behavior clause");
    }
    if contract.side_effects.is_empty() {
        bail!("theorem contract requires at least one side effects clause");
    }
    if contract.preconditions.is_empty() {
        bail!("theorem contract requires at least one precondition");
    }

    Ok(())
}

fn equivalence_contract_from_theorem_contract(contract: &TheoremContract) -> EquivalenceContract {
    EquivalenceContract {
        observable_outputs: contract.observable_outputs.clone(),
        error_behavior: contract.error_behavior.clone(),
        side_effects: contract.side_effects.clone(),
        preconditions: contract.preconditions.clone(),
    }
}

fn semantic_diff_output_with_reference(
    context: &SemanticDiffBuildContext<'_>,
) -> SemanticDiffOutput {
    let delta = semantic_delta(
        context.reference_snapshot,
        context.snapshot,
        context.max_changed_scope_items,
    );
    let (equivalence_stage, status, summary, evidence_receipts, theorem_artifacts) =
        semantic_diff_stage_result(context, &delta);
    let mut recommended_next_steps = semantic_diff_next_steps(&delta);
    if let Some(command) = theorem_artifacts.replay_command.as_ref() {
        recommended_next_steps.push(command.clone());
    }

    SemanticDiffOutput {
        snapshot_id: context.metadata.snapshot_binding.snapshot_id.clone(),
        readiness_state: readiness_label(context.metadata.readiness_state).to_string(),
        reference: context.reference.to_string(),
        reference_snapshot_id: Some(context.reference_snapshot.snapshot_id.clone()),
        status,
        summary,
        equivalence_stage,
        equivalence_contract: context.equivalence_contract.clone(),
        evidence_receipts,
        theorem_contract: theorem_artifacts.contract,
        theorem_run_status: theorem_artifacts.run_status,
        theorem_obligations: theorem_artifacts.obligations,
        theorem_certificates: theorem_artifacts.certificates,
        theorem_replay_artifact: theorem_artifacts.replay_artifact,
        theorem_replay_command: theorem_artifacts.replay_command,
        planned_backlog: semantic_equivalence_backlog(equivalence_stage),
        changed_scope: delta.changed_scope.clone(),
        added_files: delta.added_files.clone(),
        removed_files: delta.removed_files.clone(),
        modified_files: delta.modified_files.clone(),
        added_symbol_facts: delta.added_symbol_facts,
        removed_symbol_facts: delta.removed_symbol_facts,
        added_import_facts: delta.added_import_facts,
        removed_import_facts: delta.removed_import_facts,
        added_verification_targets: delta.added_verification_targets.clone(),
        removed_verification_targets: delta.removed_verification_targets.clone(),
        recommended_next_steps,
    }
}

type SemanticStageResult = (
    EquivalenceStage,
    String,
    String,
    Vec<EquivalenceEvidenceReceipt>,
    TheoremStageArtifacts,
);

fn semantic_diff_stage_result(
    context: &SemanticDiffBuildContext<'_>,
    delta: &SemanticDelta,
) -> SemanticStageResult {
    match context.requested_stage {
        CliSemanticDiffStage::L0StructuralDelta => semantic_diff_l0_stage(delta, context),
        CliSemanticDiffStage::L1RustBounded => semantic_diff_l1_stage(delta, context),
        CliSemanticDiffStage::L2RelationalSemantic => semantic_diff_l2_stage(delta, context),
        CliSemanticDiffStage::L3TheoremContract => semantic_diff_l3_stage(delta, context),
    }
}

fn semantic_diff_l0_stage(
    delta: &SemanticDelta,
    context: &SemanticDiffBuildContext<'_>,
) -> SemanticStageResult {
    let status = if delta.has_changes() {
        "implemented".to_string()
    } else {
        "no_structural_change".to_string()
    };
    let summary = semantic_diff_summary(
        delta,
        &context.reference_snapshot.snapshot_id,
        &context.snapshot.snapshot_id,
    );

    (
        EquivalenceStage::L0StructuralDelta,
        status,
        summary,
        vec![l0_structural_delta_receipt(delta)],
        TheoremStageArtifacts::default(),
    )
}

fn semantic_diff_l1_stage(
    delta: &SemanticDelta,
    context: &SemanticDiffBuildContext<'_>,
) -> SemanticStageResult {
    let l1_receipt = l1_rust_bounded_receipt(
        context.repo_root,
        context.snapshot,
        delta,
        context.l1_policy,
    );
    let summary = l1_summary(
        &l1_receipt,
        delta,
        &context.reference_snapshot.snapshot_id,
        &context.snapshot.snapshot_id,
    );
    (
        EquivalenceStage::L1BoundedFormal,
        l1::l1_status_label(l1_receipt.status).to_string(),
        summary,
        vec![l1_receipt],
        TheoremStageArtifacts::default(),
    )
}

fn semantic_diff_l2_stage(
    delta: &SemanticDelta,
    context: &SemanticDiffBuildContext<'_>,
) -> SemanticStageResult {
    let l2_receipt = l2_relational_semantic_receipt(
        context.reference_snapshot,
        context.snapshot,
        delta,
        context.l2_policy,
    );
    let summary = l2_summary(
        &l2_receipt,
        delta,
        &context.reference_snapshot.snapshot_id,
        &context.snapshot.snapshot_id,
    );
    (
        EquivalenceStage::L2RelationalSemantic,
        l2_status_label(l2_receipt.status).to_string(),
        summary,
        vec![l2_receipt],
        TheoremStageArtifacts::default(),
    )
}

fn semantic_diff_l3_stage(
    delta: &SemanticDelta,
    context: &SemanticDiffBuildContext<'_>,
) -> SemanticStageResult {
    let Some(theorem_contract) = context.theorem_contract else {
        let summary = "theorem stage requires an explicit theorem contract".to_string();
        return (
            EquivalenceStage::L3TheoremContract,
            "inconclusive_under_contract".to_string(),
            summary,
            vec![theorem_stage_missing_contract_receipt(
                &context.theorem_policy,
            )],
            TheoremStageArtifacts::default(),
        );
    };
    let theorem_artifacts = theorem_stage_artifacts(
        context.repo_root,
        context.reference_snapshot,
        context.snapshot,
        delta,
        &context.theorem_policy,
        theorem_contract,
    );
    let theorem_receipt = theorem_stage_receipt(&theorem_artifacts, &context.theorem_policy);
    let mut evidence_receipts = vec![theorem_receipt];
    evidence_receipts.extend(theorem_artifacts.extra_receipts.clone());
    let summary = theorem_stage_summary(
        &theorem_artifacts,
        delta,
        &context.reference_snapshot.snapshot_id,
        &context.snapshot.snapshot_id,
    );

    (
        EquivalenceStage::L3TheoremContract,
        theorem_run_status_label(
            theorem_artifacts
                .run_status
                .unwrap_or(TheoremRunStatus::InconclusiveUnderContract),
        )
        .to_string(),
        summary,
        evidence_receipts,
        theorem_artifacts,
    )
}

fn semantic_diff_output_missing_reference(
    repo_root: &Path,
    metadata: &repobrain_serving::ServingMetadata,
    reference: &str,
    requested_stage: CliSemanticDiffStage,
    equivalence_contract: EquivalenceContract,
    theorem_contract: Option<TheoremContract>,
) -> SemanticDiffOutput {
    let equivalence_stage = equivalence_stage_for_cli(requested_stage);
    let reference_for_scan = reference.to_string();
    let stage_flag = semantic_stage_cli_label(requested_stage);

    SemanticDiffOutput {
        snapshot_id: metadata.snapshot_binding.snapshot_id.clone(),
        readiness_state: readiness_label(metadata.readiness_state).to_string(),
        reference: reference.to_string(),
        reference_snapshot_id: None,
        status: "reference_snapshot_missing".to_string(),
        summary: format!(
            "reference snapshot `{reference_for_scan}` is not available; semantic diff compares stored snapshots only"
        ),
        equivalence_stage,
        equivalence_contract,
        evidence_receipts: vec![missing_reference_receipt(
            equivalence_stage,
            backend_label_for_stage(requested_stage),
            &reference_for_scan,
        )],
        theorem_contract,
        theorem_run_status: None,
        theorem_obligations: Vec::new(),
        theorem_certificates: Vec::new(),
        theorem_replay_artifact: None,
        theorem_replay_command: None,
        planned_backlog: semantic_equivalence_backlog(equivalence_stage),
        changed_scope: Vec::new(),
        added_files: Vec::new(),
        removed_files: Vec::new(),
        modified_files: Vec::new(),
        added_symbol_facts: 0,
        removed_symbol_facts: 0,
        added_import_facts: 0,
        removed_import_facts: 0,
        added_verification_targets: Vec::new(),
        removed_verification_targets: Vec::new(),
        recommended_next_steps: vec![
            format!(
                "repobrain scan --revision {reference_for_scan} --repo-root {}",
                repo_root.display()
            ),
            format!(
                "repobrain what-changed-semantically --ref {reference_for_scan} --stage {stage_flag} --repo-root {}",
                repo_root.display()
            ),
        ],
    }
}

fn semantic_stage_cli_label(stage: CliSemanticDiffStage) -> &'static str {
    match stage {
        CliSemanticDiffStage::L0StructuralDelta => "l0-structural-delta",
        CliSemanticDiffStage::L1RustBounded => "l1-rust-bounded",
        CliSemanticDiffStage::L2RelationalSemantic => "l2-relational-semantic",
        CliSemanticDiffStage::L3TheoremContract => "l3-theorem-contract",
    }
}

pub(super) fn semantic_delta(
    reference: &RepositoryInventorySnapshot,
    target: &RepositoryInventorySnapshot,
    max_changed_scope_items: usize,
) -> SemanticDelta {
    let file_delta = file_inventory_delta(reference, target);
    let mut changed_scope = file_delta.changed_scope;
    let symbol_delta = symbol_fact_delta(reference, target, &mut changed_scope);
    let import_delta = import_fact_delta(reference, target, &mut changed_scope);
    let verification_target_delta = verification_target_delta(reference, target);

    SemanticDelta {
        changed_scope: changed_scope
            .into_iter()
            .take(max_changed_scope_items)
            .collect(),
        added_files: file_delta.added_files,
        removed_files: file_delta.removed_files,
        modified_files: file_delta.modified_files,
        added_symbol_facts: symbol_delta.added,
        removed_symbol_facts: symbol_delta.removed,
        added_import_facts: import_delta.added,
        removed_import_facts: import_delta.removed,
        added_verification_targets: verification_target_delta.added,
        removed_verification_targets: verification_target_delta.removed,
    }
}

#[derive(Debug)]
struct FileInventoryDelta {
    added_files: Vec<String>,
    removed_files: Vec<String>,
    modified_files: Vec<String>,
    changed_scope: BTreeSet<String>,
}

fn file_inventory_delta(
    reference: &RepositoryInventorySnapshot,
    target: &RepositoryInventorySnapshot,
) -> FileInventoryDelta {
    let reference_file_meta = reference
        .files
        .iter()
        .map(|file| {
            (
                file.relative_path.clone(),
                (
                    file.size_bytes,
                    file.language.clone(),
                    file.content_hash.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let target_file_meta = target
        .files
        .iter()
        .map(|file| {
            (
                file.relative_path.clone(),
                (
                    file.size_bytes,
                    file.language.clone(),
                    file.content_hash.clone(),
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let reference_files = reference_file_meta.keys().cloned().collect::<BTreeSet<_>>();
    let target_files = target_file_meta.keys().cloned().collect::<BTreeSet<_>>();

    let added_files = target_files
        .difference(&reference_files)
        .cloned()
        .collect::<Vec<_>>();
    let removed_files = reference_files
        .difference(&target_files)
        .cloned()
        .collect::<Vec<_>>();
    let modified_files = target_files
        .intersection(&reference_files)
        .filter(|path| reference_file_meta.get(*path) != target_file_meta.get(*path))
        .cloned()
        .collect::<Vec<_>>();

    let mut changed_scope = BTreeSet::new();
    changed_scope.extend(added_files.iter().cloned());
    changed_scope.extend(removed_files.iter().cloned());
    changed_scope.extend(modified_files.iter().cloned());

    FileInventoryDelta {
        added_files,
        removed_files,
        modified_files,
        changed_scope,
    }
}

#[derive(Debug)]
struct FactDelta {
    added: usize,
    removed: usize,
}

fn symbol_fact_delta(
    reference: &RepositoryInventorySnapshot,
    target: &RepositoryInventorySnapshot,
    changed_scope: &mut BTreeSet<String>,
) -> FactDelta {
    let reference_symbol_paths = reference
        .symbols
        .iter()
        .map(|symbol| (symbol.fact_id.clone(), symbol.relative_path.clone()))
        .collect::<BTreeMap<_, _>>();
    let target_symbol_paths = target
        .symbols
        .iter()
        .map(|symbol| (symbol.fact_id.clone(), symbol.relative_path.clone()))
        .collect::<BTreeMap<_, _>>();

    let reference_symbol_ids = reference_symbol_paths
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let target_symbol_ids = target_symbol_paths.keys().cloned().collect::<BTreeSet<_>>();
    let added_symbol_ids = target_symbol_ids
        .difference(&reference_symbol_ids)
        .cloned()
        .collect::<Vec<_>>();
    let removed_symbol_ids = reference_symbol_ids
        .difference(&target_symbol_ids)
        .cloned()
        .collect::<Vec<_>>();

    for fact_id in &added_symbol_ids {
        if let Some(path) = target_symbol_paths.get(fact_id) {
            let _ = changed_scope.insert(path.clone());
        }
    }
    for fact_id in &removed_symbol_ids {
        if let Some(path) = reference_symbol_paths.get(fact_id) {
            let _ = changed_scope.insert(path.clone());
        }
    }

    FactDelta {
        added: added_symbol_ids.len(),
        removed: removed_symbol_ids.len(),
    }
}

fn import_fact_delta(
    reference: &RepositoryInventorySnapshot,
    target: &RepositoryInventorySnapshot,
    changed_scope: &mut BTreeSet<String>,
) -> FactDelta {
    let reference_import_paths = reference
        .imports
        .iter()
        .map(|import| {
            (
                import.fact_id.clone(),
                (import.importer_path.clone(), import.resolved_path.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let target_import_paths = target
        .imports
        .iter()
        .map(|import| {
            (
                import.fact_id.clone(),
                (import.importer_path.clone(), import.resolved_path.clone()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let reference_import_ids = reference_import_paths
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let target_import_ids = target_import_paths.keys().cloned().collect::<BTreeSet<_>>();
    let added_import_ids = target_import_ids
        .difference(&reference_import_ids)
        .cloned()
        .collect::<Vec<_>>();
    let removed_import_ids = reference_import_ids
        .difference(&target_import_ids)
        .cloned()
        .collect::<Vec<_>>();

    for fact_id in &added_import_ids {
        if let Some((importer_path, resolved_path)) = target_import_paths.get(fact_id) {
            let _ = changed_scope.insert(importer_path.clone());
            if let Some(resolved_path) = resolved_path {
                let _ = changed_scope.insert(resolved_path.clone());
            }
        }
    }
    for fact_id in &removed_import_ids {
        if let Some((importer_path, resolved_path)) = reference_import_paths.get(fact_id) {
            let _ = changed_scope.insert(importer_path.clone());
            if let Some(resolved_path) = resolved_path {
                let _ = changed_scope.insert(resolved_path.clone());
            }
        }
    }

    FactDelta {
        added: added_import_ids.len(),
        removed: removed_import_ids.len(),
    }
}

#[derive(Debug)]
struct VerificationTargetDelta {
    added: Vec<String>,
    removed: Vec<String>,
}

fn verification_target_delta(
    reference: &RepositoryInventorySnapshot,
    target: &RepositoryInventorySnapshot,
) -> VerificationTargetDelta {
    let reference_verification_targets = reference
        .verification_targets()
        .iter()
        .map(render_verification_target)
        .collect::<BTreeSet<_>>();
    let target_verification_targets = target
        .verification_targets()
        .iter()
        .map(render_verification_target)
        .collect::<BTreeSet<_>>();

    VerificationTargetDelta {
        added: target_verification_targets
            .difference(&reference_verification_targets)
            .cloned()
            .collect(),
        removed: reference_verification_targets
            .difference(&target_verification_targets)
            .cloned()
            .collect(),
    }
}

fn render_verification_target(target: &repobrain_ingest::IndexedVerificationTarget) -> String {
    let rendered_command = target.command.join(" ");
    let rendered_with_directory = if target.working_directory == "." {
        rendered_command
    } else {
        format!("{} (in {})", rendered_command, target.working_directory)
    };
    let scope_label = if target.scope_root.is_empty() {
        "<repo>"
    } else {
        target.scope_root.as_str()
    };

    format!("{rendered_with_directory} [scope:{scope_label}]")
}

fn semantic_diff_summary(
    delta: &SemanticDelta,
    reference_snapshot_id: &str,
    target_snapshot_id: &str,
) -> String {
    if !delta.has_changes() {
        return format!(
            "no deterministic structural delta observed between `{target_snapshot_id}` and `{reference_snapshot_id}`"
        );
    }

    let verification_target_delta =
        delta.added_verification_targets.len() + delta.removed_verification_targets.len();

    format!(
        "deterministic structural delta between `{target_snapshot_id}` and `{reference_snapshot_id}`: {} file add, {} file remove, {} file metadata modify, {} symbol fact add, {} symbol fact remove, {} import fact add, {} import fact remove, {} verification target change",
        delta.added_files.len(),
        delta.removed_files.len(),
        delta.modified_files.len(),
        delta.added_symbol_facts,
        delta.removed_symbol_facts,
        delta.added_import_facts,
        delta.removed_import_facts,
        verification_target_delta,
    )
}

fn semantic_diff_next_steps(delta: &SemanticDelta) -> Vec<String> {
    let mut next_steps = Vec::new();

    if let Some(primary_scope) = delta.changed_scope.first() {
        next_steps.push(format!("repobrain blast-radius {primary_scope}"));
        next_steps.push(format!(
            "repobrain list-invariants --scope {primary_scope} --token-budget 4096"
        ));
    }

    next_steps.push(
        "repobrain get-brief --goal \"safe edit\" --scope <path-or-symbol> --token-budget 4096"
            .to_string(),
    );

    next_steps
}

fn semantic_equivalence_contract() -> EquivalenceContract {
    EquivalenceContract {
        observable_outputs: vec![
            "process exit code".to_string(),
            "stdout and stderr payloads".to_string(),
            "persisted snapshot artifacts under .repobrain/snapshots".to_string(),
        ],
        error_behavior: vec![
            "non-zero exit or explicit error payload must be treated as behavior change"
                .to_string(),
            "reference snapshot missing is a distinct non-proof state".to_string(),
        ],
        side_effects: vec![
            "snapshot inventory write operations".to_string(),
            "filesystem reads during deterministic extraction".to_string(),
        ],
        preconditions: vec![
            "both snapshots are produced by deterministic RepoBrain ingest".to_string(),
            "comparison is bounded to stored snapshot evidence (no hidden runtime state)"
                .to_string(),
        ],
    }
}

fn semantic_equivalence_backlog(stage: EquivalenceStage) -> Vec<String> {
    let current_focus = match stage {
        EquivalenceStage::L0StructuralDelta => {
            "current stage focus: keep L0 deterministic while deeper lanes stay additive"
        }
        EquivalenceStage::L1BoundedFormal => {
            "current stage focus: keep targeted Kani obligations bounded and evidence-rich"
        }
        EquivalenceStage::L2RelationalSemantic => {
            "current stage focus: keep alignment and relational witnesses precise under explicit bounds"
        }
        EquivalenceStage::L3TheoremContract => {
            "current stage focus: keep theorem receipts bounded while compiler-IR lanes grow deeper"
        }
    };

    vec![
        format!(
            "Iteration 1: L0 hardening, widen deterministic structural sensitivity and default verification guidance; {current_focus}."
        ),
        "Iteration 2: L1 hardening, deepen obligation targeting, harness synthesis, and large-scope triage without regressing bounded execution.".to_string(),
        "Iteration 3: L2 hardening, improve behavioral alignment precision/recall and solver-backed relational witnesses for high-risk pairs.".to_string(),
        "Iteration 4: L3 hardening, expand LLVM SSA/IVL obligations with bounded memory, pointer, and call summaries plus stricter compiler-IR translation validation.".to_string(),
        "Iteration 5: Cross-stage hardening, converge replay, perf ceilings, no-overclaim audits, and residual-risk reporting across L0/L1/L2/L3.".to_string(),
    ]
}

fn l0_structural_delta_receipt(delta: &SemanticDelta) -> EquivalenceEvidenceReceipt {
    let status = if delta.has_changes() {
        EquivalenceEvidenceStatus::ObservedDifference
    } else {
        EquivalenceEvidenceStatus::NoDifferenceObserved
    };

    EquivalenceEvidenceReceipt {
        backend: "snapshot_structural_delta".to_string(),
        stage: EquivalenceStage::L0StructuralDelta,
        status,
        solver: None,
        bounds: Some("files,symbol_facts,import_facts,verification_targets".to_string()),
        timeout_ms: None,
        assumptions: vec![
            "evidence is structural and deterministic; this is not semantic equivalence proof"
                .to_string(),
            "comparison requires stored target and reference snapshots".to_string(),
        ],
        witness: semantic_delta_witness(delta),
    }
}

fn missing_reference_receipt(
    stage: EquivalenceStage,
    backend: &str,
    reference: &str,
) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: backend.to_string(),
        stage,
        status: EquivalenceEvidenceStatus::ReferenceSnapshotMissing,
        solver: None,
        bounds: Some("reference_snapshot_artifact_required".to_string()),
        timeout_ms: None,
        assumptions: vec![
            "semantic diff requires a persisted reference snapshot artifact".to_string(),
            format!("reference `{reference}` was not found in .repobrain/snapshots"),
        ],
        witness: None,
    }
}

fn equivalence_stage_for_cli(stage: CliSemanticDiffStage) -> EquivalenceStage {
    match stage {
        CliSemanticDiffStage::L0StructuralDelta => EquivalenceStage::L0StructuralDelta,
        CliSemanticDiffStage::L1RustBounded => EquivalenceStage::L1BoundedFormal,
        CliSemanticDiffStage::L2RelationalSemantic => EquivalenceStage::L2RelationalSemantic,
        CliSemanticDiffStage::L3TheoremContract => EquivalenceStage::L3TheoremContract,
    }
}

fn backend_label_for_stage(stage: CliSemanticDiffStage) -> &'static str {
    match stage {
        CliSemanticDiffStage::L0StructuralDelta => "snapshot_structural_delta",
        CliSemanticDiffStage::L1RustBounded => "kani_rust_bounded",
        CliSemanticDiffStage::L2RelationalSemantic => "l2_relational_semantic",
        CliSemanticDiffStage::L3TheoremContract => "theorem_obligation_compiler",
    }
}

fn l1_policy_from_args(args: &WhatChangedSemanticallyArgs) -> L1RustBoundedPolicy {
    L1RustBoundedPolicy {
        max_rust_files: args.l1_max_rust_files.max(1),
        max_functions: args.l1_max_functions.max(1),
        timeout_ms: args.l1_timeout_ms.max(250),
        kani_mode: args.l1_kani_mode,
    }
}

fn l2_policy_from_args(args: &WhatChangedSemanticallyArgs) -> L2RelationalPolicy {
    L2RelationalPolicy {
        max_pairs: args.l2_max_pairs.max(1),
        max_candidate_functions: args.l2_max_candidate_functions.max(1),
        timeout_ms: args.l2_timeout_ms.max(250),
        alignment_mode: args.l2_alignment_mode,
        min_alignment_score: args.l2_min_alignment_score.min(100),
    }
}

fn theorem_policy_from_args(args: &WhatChangedSemanticallyArgs) -> TheoremPolicy {
    TheoremPolicy {
        max_obligations: args.theorem_max_obligations.max(1),
        max_candidate_functions: args.theorem_max_candidate_functions.max(1),
        timeout_ms: args.theorem_timeout_ms.max(250),
        flaky_retries: args.theorem_flaky_retries.max(1),
        stability_runs: args.theorem_stability_runs.max(1),
        max_parallelism: args.theorem_max_parallelism.max(1),
        solver_mode: args.theorem_solver_mode,
        solver_path: args.theorem_solver_path.clone(),
        solver_timeout_ms: args.theorem_solver_timeout_ms.max(250),
        solver_fallback_relational: args.theorem_solver_fallback_relational,
        replay_out: args.theorem_replay_out.clone(),
        persist_replay: args.theorem_persist_replay,
        translation_validation: args.theorem_translation_validation,
        alive2_path: args.theorem_alive2_path.clone(),
        translation_max_obligations: args.theorem_translation_max_obligations.max(1),
        translation_timeout_ms: args.theorem_translation_timeout_ms.max(250),
        translation_ir_mode: args.theorem_translation_ir_mode,
        translation_run_status_mode: args.theorem_translation_run_status_mode,
    }
}
