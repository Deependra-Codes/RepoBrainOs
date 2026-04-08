use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use repobrain_compiler::{CompilationInput, CompilationPolicy, ContextCompiler, DefaultCompiler};
use repobrain_domain::{
    BriefingItem, BriefingPack, ContextRequest, CoverageAudit, CoverageSlot, CoverageSlotAudit,
    CoverageStatus, EvidenceReceipt, Freshness, FreshnessImpact, ImpactSummary, LatencyClass,
    OverlayClaimScope, OverlayKind, QueryClassification, ReadinessState, TaskType,
    VerificationPlan,
};
use repobrain_graph::{FlowCapsule, SnapshotGraphStore};
use repobrain_ingest::{
    IndexedFile, IndexedSymbol, RepositoryInventorySnapshot, SymbolKind, exact_path_lookup,
    exact_symbol_lookup,
};
use repobrain_retrieval::{
    GraphExpansionRequest, GraphNeighbor, LexicalHit, LexicalQuery, SnapshotLexicalIndexCache,
    coverage_requires_targeted_second_pass, expand_import_graph,
};
use repobrain_serving::{
    HotPathPolicy, ReadinessAssessment, ServingMetadataInput, SnapshotKey, build_serving_metadata,
};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct SnapshotBrokerInput<'a> {
    pub request: ContextRequest,
    pub snapshot: &'a RepositoryInventorySnapshot,
    pub readiness_assessment: ReadinessAssessment,
    pub overlay_kind: OverlayKind,
    pub claim_scope: OverlayClaimScope,
    pub overlay_hash: Option<String>,
    pub touched_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotContextBroker {
    hot_path_policy: HotPathPolicy,
    lexical_cache: Arc<SnapshotLexicalIndexCache>,
    decision_path_cache: Arc<SnapshotDecisionPathCache>,
}

impl SnapshotContextBroker {
    #[must_use]
    pub fn new(hot_path_policy: HotPathPolicy) -> Self {
        Self {
            hot_path_policy,
            lexical_cache: Arc::new(SnapshotLexicalIndexCache::default()),
            decision_path_cache: Arc::new(SnapshotDecisionPathCache::default()),
        }
    }

    /// Compiles a deterministic snapshot-backed briefing pack for a scoped request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request does not include a scope hint or the
    /// scope cannot be anchored to an exact path or exact symbol in the
    /// provided snapshot.
    #[allow(clippy::too_many_lines)]
    pub fn get_brief(&self, input: SnapshotBrokerInput<'_>) -> Result<BriefingPack, BrokerError> {
        let scope = input
            .request
            .scope_hint
            .clone()
            .ok_or(BrokerError::MissingScopeHint)?;
        let routing = self.hot_path_policy.route(&input.request);
        let query_classification = classify_request(&input.request);
        let repo_shape = repo_shape_bucket(input.snapshot);
        let exact_path_hit = exact_path_lookup(input.snapshot, &scope);
        let exact_symbol_hits = exact_symbol_lookup(input.snapshot, &scope);
        let mut slot_cost_ledger = SlotCostLedger::default();
        let mut lexical_hits = Vec::new();

        if exact_path_hit.is_none() && exact_symbol_hits.is_empty() {
            let lexical_started = Instant::now();
            lexical_hits = self
                .lexical_cache
                .search(
                    input.snapshot,
                    &LexicalQuery {
                        text: scope.clone(),
                        scope: None,
                        limit: routing.max_candidates,
                    },
                )
                .map_err(|error| BrokerError::LexicalSearchFailure(error.to_string()))?;
            if lexical_hits.is_empty() {
                return Err(BrokerError::UnknownScope(scope));
            }
            slot_cost_ledger.record_stage_cost(
                &lexical_stage_slots(query_classification),
                elapsed_ms(lexical_started),
            );
        }
        let mut lexical_receipts =
            lexical_hits_for_evidence(&lexical_hits, &input.snapshot.captured_at);

        let graph = SnapshotGraphStore::new(input.snapshot);
        let report = graph.blast_radius_with_decision_mode(
            input.request.scope_hint.as_deref().unwrap_or_default(),
            query_classification == QueryClassification::DecisionWhy,
        );
        let report_decision_paths = decision_document_paths_from_evidence(&report.evidence);
        let verification_plan = report.verification_plan.clone();
        let metadata = serving_metadata_for_broker(&input, &verification_plan);
        let packing_budget = packing_budget(routing.latency_class, input.request.token_budget);
        let freshness = freshness_for(metadata.readiness_state);

        let structural_started = Instant::now();
        let mut expansion = expand_import_graph(
            input.snapshot,
            &GraphExpansionRequest {
                seed_paths: seed_paths_for_expansion(
                    exact_path_hit,
                    exact_symbol_hits,
                    &lexical_hits,
                ),
                max_hops: routing.max_graph_hops,
                max_candidates: routing.max_candidates,
            },
        );
        slot_cost_ledger.record_stage_cost(
            &structural_stage_slots(query_classification),
            elapsed_ms(structural_started),
        );
        let mut suggested_files = merge_suggested_files(
            collect_suggested_files(&report.impacted_nodes, packing_budget.suggested_files),
            &lexical_hits,
            &expansion.neighbors,
            packing_budget.suggested_files,
        );
        let mut anchor_paths =
            collect_anchor_paths_with_lexical(exact_path_hit, exact_symbol_hits, &lexical_hits);
        let mut lexical_anchor_count = lexical_hits.len();
        let decision_started = Instant::now();
        let mut relevant_decisions = decision_evidence_for_query(
            query_classification,
            &DecisionEvidenceQueryContext {
                snapshot: input.snapshot,
                scope: input.request.scope_hint.as_deref().unwrap_or_default(),
                anchor_paths: &anchor_paths,
                suggested_files: &suggested_files,
                lexical_cache: self.lexical_cache.as_ref(),
                decision_path_cache: self.decision_path_cache.as_ref(),
                report_decision_paths: &report_decision_paths,
            },
        );
        slot_cost_ledger.record_stage_cost(
            &decision_stage_slots(query_classification),
            elapsed_ms(decision_started),
        );
        let mut impact_summary = build_impact_summary(
            input.request.scope_hint.as_deref().unwrap_or_default(),
            &suggested_files,
        );
        let mut coverage_audit = build_coverage_audit(&CoverageAuditContext {
            query_classification,
            snapshot: input.snapshot,
            readiness_state: metadata.readiness_state,
            exact_path_hit,
            exact_symbol_hits,
            relationship_count: report.relationships.len(),
            flow_capsules: &report.flow_capsules,
            verification_plan: &verification_plan,
            impact_summary: impact_summary.as_ref(),
            relevant_decisions: &relevant_decisions,
            suggested_files: &suggested_files,
            anchor_path_count: anchor_paths.len(),
            lexical_anchor_count,
            slot_cost_ledger: Some(&slot_cost_ledger),
        });
        let first_pass_coverage_audit = coverage_audit.clone();
        let mut second_pass_stop_reason = None;
        let adaptive_key = AdaptiveSecondPassKey::new(query_classification, repo_shape);
        let mut adaptive_model =
            load_adaptive_second_pass_model(input.snapshot.root.as_str(), &adaptive_key);

        if should_run_targeted_second_pass(
            routing.allow_second_pass,
            query_classification,
            &coverage_audit,
        ) {
            let adaptive_budget = adaptive_model.recommend_budget(
                query_classification,
                repo_shape,
                second_pass_candidate_budget(routing.max_candidates),
                second_pass_hop_budget(routing.max_graph_hops),
                routing.latency_class,
            );
            let second_pass_started = Instant::now();
            let second_pass_queries = second_pass_subqueries(
                &scope,
                input.request.question.as_str(),
                query_classification,
                &coverage_audit,
            );
            let frontier_floor =
                class_specific_frontier_floor(routing.latency_class, query_classification);
            let mut frontier_controller = RetrievalFrontierController::new(
                frontier_floor,
                adaptive_model.expected_healed_slots_per_ms(query_classification, repo_shape),
            );
            let mut remaining_candidate_budget = adaptive_budget.max_candidates;

            if !frontier_controller.should_continue() {
                second_pass_stop_reason = Some(format!(
                    "retrieval frontier forecast {:.4} healed-slot/ms below class floor {:.4}",
                    frontier_controller.expected_healed_slots_per_ms(),
                    frontier_floor
                ));
            }

            for (index, subquery) in second_pass_queries.iter().enumerate() {
                if second_pass_stop_reason.is_some() || remaining_candidate_budget == 0 {
                    break;
                }
                if !frontier_controller.should_continue() {
                    second_pass_stop_reason = Some(format!(
                        "retrieval frontier forecast {:.4} healed-slot/ms below class floor {:.4}",
                        frontier_controller.expected_healed_slots_per_ms(),
                        frontier_floor
                    ));
                    break;
                }

                let queries_left = second_pass_queries.len().saturating_sub(index).max(1);
                let per_query_limit = (remaining_candidate_budget / queries_left)
                    .max(4)
                    .min(remaining_candidate_budget);
                if per_query_limit == 0 {
                    break;
                }

                let lexical_second_pass_started = Instant::now();
                let widened_lexical_hits = self
                    .lexical_cache
                    .search(
                        input.snapshot,
                        &LexicalQuery {
                            text: subquery.text.clone(),
                            scope: None,
                            limit: per_query_limit,
                        },
                    )
                    .map_err(|error| BrokerError::LexicalSearchFailure(error.to_string()))?;
                let lexical_elapsed_ms = elapsed_ms(lexical_second_pass_started);
                slot_cost_ledger.record_stage_cost(
                    &lexical_stage_slots_for_subquery(query_classification, subquery.slot),
                    lexical_elapsed_ms,
                );
                merge_lexical_hits(
                    &mut lexical_hits,
                    &widened_lexical_hits,
                    adaptive_budget.max_candidates,
                );
                lexical_receipts =
                    lexical_hits_for_evidence(&lexical_hits, &input.snapshot.captured_at);
                suggested_files = merge_suggested_files(
                    collect_suggested_files(&report.impacted_nodes, packing_budget.suggested_files),
                    &lexical_hits,
                    &expansion.neighbors,
                    packing_budget.suggested_files,
                );
                anchor_paths = collect_anchor_paths_with_lexical(
                    exact_path_hit,
                    exact_symbol_hits,
                    &lexical_hits,
                );
                lexical_anchor_count = lexical_hits.len();
                let decision_after_lexical_started = Instant::now();
                relevant_decisions = decision_evidence_for_query(
                    query_classification,
                    &DecisionEvidenceQueryContext {
                        snapshot: input.snapshot,
                        scope: subquery.text.as_str(),
                        anchor_paths: &anchor_paths,
                        suggested_files: &suggested_files,
                        lexical_cache: self.lexical_cache.as_ref(),
                        decision_path_cache: self.decision_path_cache.as_ref(),
                        report_decision_paths: &report_decision_paths,
                    },
                );
                slot_cost_ledger.record_stage_cost(
                    &decision_stage_slots(query_classification),
                    elapsed_ms(decision_after_lexical_started),
                );
                impact_summary = build_impact_summary(
                    input.request.scope_hint.as_deref().unwrap_or_default(),
                    &suggested_files,
                );
                let coverage_after_lexical = build_coverage_audit(&CoverageAuditContext {
                    query_classification,
                    snapshot: input.snapshot,
                    readiness_state: metadata.readiness_state,
                    exact_path_hit,
                    exact_symbol_hits,
                    relationship_count: report.relationships.len(),
                    flow_capsules: &report.flow_capsules,
                    verification_plan: &verification_plan,
                    impact_summary: impact_summary.as_ref(),
                    relevant_decisions: &relevant_decisions,
                    suggested_files: &suggested_files,
                    anchor_path_count: anchor_paths.len(),
                    lexical_anchor_count,
                    slot_cost_ledger: Some(&slot_cost_ledger),
                });
                let lexical_marginal_gain = marginal_slot_gain_per_ms(
                    &coverage_audit,
                    &coverage_after_lexical,
                    lexical_elapsed_ms,
                );
                slot_cost_ledger.record_gains_from_audits(&coverage_audit, &coverage_after_lexical);
                coverage_audit = coverage_after_lexical;
                frontier_controller.observe(lexical_marginal_gain);
                remaining_candidate_budget =
                    remaining_candidate_budget.saturating_sub(per_query_limit);

                if lexical_marginal_gain < adaptive_budget.marginal_gain_threshold {
                    second_pass_stop_reason = Some(format!(
                        "lexical stage marginal slot gain {:.4} slot/ms below threshold {:.4}",
                        lexical_marginal_gain, adaptive_budget.marginal_gain_threshold
                    ));
                    break;
                }
                if !frontier_controller.should_continue() {
                    second_pass_stop_reason = Some(format!(
                        "retrieval frontier forecast {:.4} healed-slot/ms below class floor {:.4}",
                        frontier_controller.expected_healed_slots_per_ms(),
                        frontier_floor
                    ));
                    break;
                }
                if !coverage_requires_targeted_second_pass(&coverage_audit) {
                    break;
                }
            }

            if second_pass_stop_reason.is_none()
                && coverage_requires_targeted_second_pass(&coverage_audit)
            {
                if frontier_controller.should_continue() {
                    let structural_candidate_budget = remaining_candidate_budget.max(8);
                    let structural_second_pass_started = Instant::now();
                    let second_pass_expansion = expand_import_graph(
                        input.snapshot,
                        &GraphExpansionRequest {
                            seed_paths: seed_paths_for_expansion(
                                exact_path_hit,
                                exact_symbol_hits,
                                &lexical_hits,
                            ),
                            max_hops: adaptive_budget.max_hops,
                            max_candidates: structural_candidate_budget,
                        },
                    );
                    slot_cost_ledger.record_stage_cost(
                        &structural_stage_slots(query_classification),
                        elapsed_ms(structural_second_pass_started),
                    );
                    merge_graph_neighbors(
                        &mut expansion.neighbors,
                        &second_pass_expansion.neighbors,
                        adaptive_budget.max_candidates,
                    );
                    merge_evidence_receipts(
                        &mut expansion.evidence,
                        second_pass_expansion.evidence,
                    );

                    suggested_files = merge_suggested_files(
                        collect_suggested_files(
                            &report.impacted_nodes,
                            packing_budget.suggested_files,
                        ),
                        &lexical_hits,
                        &expansion.neighbors,
                        packing_budget.suggested_files,
                    );
                    anchor_paths = collect_anchor_paths_with_lexical(
                        exact_path_hit,
                        exact_symbol_hits,
                        &lexical_hits,
                    );
                    lexical_anchor_count = lexical_hits.len();
                    let decision_after_graph_started = Instant::now();
                    let structural_scope_query = format!("{scope} {}", input.request.question);
                    relevant_decisions = decision_evidence_for_query(
                        query_classification,
                        &DecisionEvidenceQueryContext {
                            snapshot: input.snapshot,
                            scope: structural_scope_query.as_str(),
                            anchor_paths: &anchor_paths,
                            suggested_files: &suggested_files,
                            lexical_cache: self.lexical_cache.as_ref(),
                            decision_path_cache: self.decision_path_cache.as_ref(),
                            report_decision_paths: &report_decision_paths,
                        },
                    );
                    slot_cost_ledger.record_stage_cost(
                        &decision_stage_slots(query_classification),
                        elapsed_ms(decision_after_graph_started),
                    );
                    impact_summary = build_impact_summary(
                        input.request.scope_hint.as_deref().unwrap_or_default(),
                        &suggested_files,
                    );
                    let coverage_after_graph = build_coverage_audit(&CoverageAuditContext {
                        query_classification,
                        snapshot: input.snapshot,
                        readiness_state: metadata.readiness_state,
                        exact_path_hit,
                        exact_symbol_hits,
                        relationship_count: report.relationships.len(),
                        flow_capsules: &report.flow_capsules,
                        verification_plan: &verification_plan,
                        impact_summary: impact_summary.as_ref(),
                        relevant_decisions: &relevant_decisions,
                        suggested_files: &suggested_files,
                        anchor_path_count: anchor_paths.len(),
                        lexical_anchor_count,
                        slot_cost_ledger: Some(&slot_cost_ledger),
                    });
                    let structural_elapsed_ms = elapsed_ms(structural_second_pass_started);
                    let structural_marginal_gain = marginal_slot_gain_per_ms(
                        &coverage_audit,
                        &coverage_after_graph,
                        structural_elapsed_ms,
                    );
                    slot_cost_ledger
                        .record_gains_from_audits(&coverage_audit, &coverage_after_graph);
                    coverage_audit = coverage_after_graph;
                    frontier_controller.observe(structural_marginal_gain);

                    if structural_marginal_gain < adaptive_budget.marginal_gain_threshold {
                        second_pass_stop_reason = Some(format!(
                            "structural stage marginal slot gain {:.4} slot/ms below threshold {:.4}",
                            structural_marginal_gain, adaptive_budget.marginal_gain_threshold
                        ));
                    } else if !frontier_controller.should_continue() {
                        second_pass_stop_reason = Some(format!(
                            "retrieval frontier forecast {:.4} healed-slot/ms below class floor {:.4}",
                            frontier_controller.expected_healed_slots_per_ms(),
                            frontier_floor
                        ));
                    }
                } else {
                    second_pass_stop_reason = Some(format!(
                        "retrieval frontier forecast {:.4} healed-slot/ms below class floor {:.4}",
                        frontier_controller.expected_healed_slots_per_ms(),
                        frontier_floor
                    ));
                }
            }

            let second_pass_elapsed = elapsed_ms(second_pass_started);
            if let Some(reason) = second_pass_stop_reason.as_deref() {
                coverage_audit.summary =
                    format!("{} Second pass halted: {reason}.", coverage_audit.summary);
            }
            adaptive_model.observe_outcome(
                query_classification,
                repo_shape,
                &first_pass_coverage_audit,
                &coverage_audit,
                second_pass_elapsed,
            );
            if let Some(updated_stats) = adaptive_model.stats.get(&adaptive_key) {
                let _ = persist_adaptive_second_pass_stats(
                    input.snapshot.root.as_str(),
                    &adaptive_key,
                    updated_stats,
                );
            }
        }

        let evidence_index = merge_evidence_index(
            collect_evidence_index(&report.evidence, &report.flow_capsules),
            &lexical_receipts,
            &expansion.evidence,
        );
        let must_know = collect_must_know(&MustKnowContext {
            scope: input.request.scope_hint.as_deref().unwrap_or_default(),
            exact_path_hit,
            exact_symbol_hits,
            lexical_hits: &lexical_hits,
            suggested_files: &suggested_files,
            flow_capsules: &report.flow_capsules,
            evidence_index: &evidence_index,
            verification_plan: &verification_plan,
            freshness,
            coverage_audit: &coverage_audit,
        });
        let fragile_zones = collect_fragile_zones(
            &suggested_files,
            &anchor_paths,
            &verification_plan.coverage_gaps,
            packing_budget.fragile_zones,
        );
        let relevant_flows = collect_relevant_flows(&report.flow_capsules);
        let compiler = DefaultCompiler::new(CompilationPolicy {
            max_brief_items: packing_budget.brief_items,
        });

        Ok(compiler.compile(CompilationInput {
            request: input.request,
            query_classification,
            readiness_state: metadata.readiness_state,
            snapshot_binding: metadata.snapshot_binding,
            overlay_scope: metadata.overlay_scope,
            must_know,
            relevant_flows,
            relevant_decisions,
            fragile_zones,
            do_not_break: verification_plan.invariants.clone(),
            suggested_files,
            impact_summary,
            verification_plan,
            coverage_audit,
            evidence_index,
        }))
    }
}

impl Default for SnapshotContextBroker {
    fn default() -> Self {
        Self::new(HotPathPolicy::default())
    }
}

fn serving_metadata_for_broker(
    input: &SnapshotBrokerInput<'_>,
    verification_plan: &VerificationPlan,
) -> repobrain_serving::ServingMetadata {
    let snapshot_key = SnapshotKey::new(
        input.snapshot.root.clone(),
        input.snapshot.revision.clone(),
        input.overlay_hash.clone(),
    );

    build_serving_metadata(
        &snapshot_key,
        ServingMetadataInput {
            readiness_assessment: input.readiness_assessment.clone(),
            overlay_kind: input.overlay_kind,
            claim_scope: input.claim_scope,
            overlay_hash: input.overlay_hash.clone(),
            touched_paths: input.touched_paths.clone(),
            required_checks: verification_plan.required_checks.clone(),
            recommended_checks: verification_plan.recommended_checks.clone(),
            invariants: verification_plan.invariants.clone(),
            coverage_gaps: verification_plan.coverage_gaps.clone(),
            stop_conditions: verification_plan.stop_conditions.clone(),
        },
    )
}

fn decision_document_paths_from_evidence(evidence: &[EvidenceReceipt]) -> Vec<String> {
    let mut paths = evidence
        .iter()
        .filter(|receipt| receipt.source_type == "decision_document")
        .map(|receipt| normalize_path(&receipt.source_ref))
        .collect::<Vec<_>>();
    paths.sort_unstable();
    paths.dedup();

    paths
}

struct DecisionEvidenceQueryContext<'a> {
    snapshot: &'a RepositoryInventorySnapshot,
    scope: &'a str,
    anchor_paths: &'a BTreeSet<String>,
    suggested_files: &'a [String],
    lexical_cache: &'a SnapshotLexicalIndexCache,
    decision_path_cache: &'a SnapshotDecisionPathCache,
    report_decision_paths: &'a [String],
}

fn decision_evidence_for_query(
    query_classification: QueryClassification,
    context: &DecisionEvidenceQueryContext<'_>,
) -> Vec<String> {
    if query_classification != QueryClassification::DecisionWhy {
        return Vec::new();
    }

    collect_relevant_decisions(context, 4)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrokerError {
    #[error("get_brief requires a scope hint for the current deterministic slice")]
    MissingScopeHint,
    #[error("scope `{0}` was not found in the current snapshot")]
    UnknownScope(String),
    #[error("lexical retrieval failed: {0}")]
    LexicalSearchFailure(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackingBudget {
    brief_items: usize,
    suggested_files: usize,
    fragile_zones: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RepoShapeBucket {
    Tiny,
    Small,
    Medium,
    LargeDense,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct AdaptiveSecondPassKey {
    query_classification: &'static str,
    repo_shape: RepoShapeBucket,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct AdaptiveSecondPassStats {
    observations: u32,
    missing_slots: u32,
    healed_slots: u32,
    unresolved_slots: u32,
    gain_per_ms_ema: f64,
}

#[derive(Debug, Clone, Default)]
struct AdaptiveSecondPassModel {
    stats: BTreeMap<AdaptiveSecondPassKey, AdaptiveSecondPassStats>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AdaptiveSecondPassBudget {
    max_candidates: usize,
    max_hops: u8,
    marginal_gain_threshold: f64,
}

#[derive(Debug, Clone, Default)]
struct SlotCostLedger {
    cost_ms_by_slot: BTreeMap<&'static str, f64>,
    gains_by_slot: BTreeMap<&'static str, usize>,
}

#[derive(Debug, Clone)]
struct SecondPassSubquery {
    slot: CoverageSlot,
    text: String,
}

#[derive(Debug, Clone, Copy)]
struct RetrievalFrontierController {
    floor_healed_slots_per_ms: f64,
    expected_healed_slots_per_ms: f64,
    has_forecast: bool,
}

#[derive(Debug, Clone, Default)]
struct AdaptiveSecondPassStore;

#[derive(Debug, Default)]
struct SnapshotDecisionPathCache {
    paths_by_snapshot: RwLock<BTreeMap<String, Vec<String>>>,
}

#[derive(Debug, Error)]
enum AdaptiveStoreError {
    #[error("failed to create adaptive second-pass directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("adaptive second-pass sqlite error during {operation}: {source}")]
    Sqlite {
        operation: &'static str,
        source: rusqlite::Error,
    },
}

impl SnapshotDecisionPathCache {
    fn paths_for_snapshot(&self, snapshot: &RepositoryInventorySnapshot) -> Vec<String> {
        let cache_key = decision_paths_cache_key(snapshot);
        if let Ok(guard) = self.paths_by_snapshot.read()
            && let Some(cached) = guard.get(&cache_key)
        {
            return cached.clone();
        }

        let computed = build_decision_path_baseline(snapshot);
        if let Ok(mut guard) = self.paths_by_snapshot.write() {
            guard.insert(cache_key, computed.clone());
        }

        computed
    }
}

fn should_run_targeted_second_pass(
    allow_second_pass: bool,
    query_classification: QueryClassification,
    coverage_audit: &CoverageAudit,
) -> bool {
    allow_second_pass
        && query_allows_targeted_second_pass(query_classification)
        && coverage_requires_targeted_second_pass(coverage_audit)
}

fn load_adaptive_second_pass_model(
    repo_root: &str,
    key: &AdaptiveSecondPassKey,
) -> AdaptiveSecondPassModel {
    let mut model = AdaptiveSecondPassModel::default();
    if let Ok(Some(stats)) = AdaptiveSecondPassStore::load_stats(repo_root, key) {
        model.stats.insert(key.clone(), stats);
    }
    model
}

fn persist_adaptive_second_pass_stats(
    repo_root: &str,
    key: &AdaptiveSecondPassKey,
    stats: &AdaptiveSecondPassStats,
) -> bool {
    AdaptiveSecondPassStore::store_stats(repo_root, key, stats).is_ok()
}

fn query_allows_targeted_second_pass(query_classification: QueryClassification) -> bool {
    !matches!(query_classification, QueryClassification::EntityLookup)
}

fn second_pass_candidate_budget(first_pass_budget: usize) -> usize {
    first_pass_budget.saturating_mul(2).clamp(8, 96)
}

fn second_pass_hop_budget(first_pass_hops: u8) -> u8 {
    first_pass_hops.saturating_add(1).min(3)
}

fn repo_shape_bucket(snapshot: &RepositoryInventorySnapshot) -> RepoShapeBucket {
    let file_count = snapshot.files.len();
    let import_density_per_thousand = if file_count == 0 {
        0
    } else {
        snapshot.imports.len().saturating_mul(1_000) / file_count
    };

    if file_count <= 64 {
        RepoShapeBucket::Tiny
    } else if file_count <= 256 && import_density_per_thousand < 1_000 {
        RepoShapeBucket::Small
    } else if file_count <= 1_024 || import_density_per_thousand < 1_500 {
        RepoShapeBucket::Medium
    } else {
        RepoShapeBucket::LargeDense
    }
}

fn base_marginal_gain_threshold(
    latency_class: LatencyClass,
    query_classification: QueryClassification,
) -> f64 {
    let query_baseline = match query_classification {
        QueryClassification::EntityLookup => 0.03,
        QueryClassification::ArchitectureExplanation => 0.01,
        QueryClassification::SafeEdit
        | QueryClassification::BugFix
        | QueryClassification::FeatureImplementation => 0.015,
        QueryClassification::DecisionWhy => 0.008,
    };
    let latency_factor = match latency_class {
        LatencyClass::Instant => 1.3,
        LatencyClass::Interactive => 1.0,
        LatencyClass::Deep => 0.65,
        LatencyClass::Background => 1.5,
    };

    query_baseline * latency_factor
}

fn class_specific_frontier_floor(
    latency_class: LatencyClass,
    query_classification: QueryClassification,
) -> f64 {
    (base_marginal_gain_threshold(latency_class, query_classification) * 0.8).clamp(0.002, 0.05)
}

fn lexical_stage_slots(query_classification: QueryClassification) -> Vec<CoverageSlot> {
    required_slots_for(query_classification)
        .iter()
        .copied()
        .filter(|slot| {
            matches!(
                slot,
                CoverageSlot::ExactAnchor | CoverageSlot::DecisionEvidence
            )
        })
        .collect()
}

fn lexical_stage_slots_for_subquery(
    query_classification: QueryClassification,
    target_slot: CoverageSlot,
) -> Vec<CoverageSlot> {
    if required_slots_for(query_classification).contains(&target_slot) {
        return vec![target_slot];
    }

    lexical_stage_slots(query_classification)
}

fn second_pass_subqueries(
    scope: &str,
    question: &str,
    query_classification: QueryClassification,
    coverage_audit: &CoverageAudit,
) -> Vec<SecondPassSubquery> {
    let mut queries = Vec::new();
    let mut seen_texts = BTreeSet::new();
    let missing_slots = coverage_audit
        .slot_results
        .iter()
        .filter(|slot| slot.status == CoverageStatus::MissingRetrievable)
        .map(|slot| slot.slot)
        .collect::<Vec<_>>();

    for slot in missing_slots {
        let query_text = match slot {
            CoverageSlot::DecisionEvidence => format!(
                "{scope} {question} adr decision rationale tradeoff history architecture record"
            ),
            CoverageSlot::VerificationTargets => {
                format!("{scope} {question} test verify invariant regression coverage check")
            }
            CoverageSlot::StructuralContext => {
                format!("{scope} {question} import dependency caller callee module relationship")
            }
            CoverageSlot::FlowSummary => {
                format!("{scope} {question} entrypoint flow path boundary runtime")
            }
            CoverageSlot::ImpactEnvelope => {
                format!("{scope} {question} impact blast radius dependent files side effects")
            }
            CoverageSlot::ExactAnchor => format!("{scope} {question} symbol path declaration"),
        };
        if seen_texts.insert(query_text.clone()) {
            queries.push(SecondPassSubquery {
                slot,
                text: query_text,
            });
        }
    }

    if queries.is_empty() {
        let fallback = format!("{scope} {question}");
        if seen_texts.insert(fallback.clone()) {
            queries.push(SecondPassSubquery {
                slot: match query_classification {
                    QueryClassification::DecisionWhy => CoverageSlot::DecisionEvidence,
                    _ => CoverageSlot::ExactAnchor,
                },
                text: fallback,
            });
        }
    }

    queries
}

fn structural_stage_slots(query_classification: QueryClassification) -> Vec<CoverageSlot> {
    required_slots_for(query_classification)
        .iter()
        .copied()
        .filter(|slot| {
            matches!(
                slot,
                CoverageSlot::StructuralContext
                    | CoverageSlot::FlowSummary
                    | CoverageSlot::ImpactEnvelope
            )
        })
        .collect()
}

fn decision_stage_slots(query_classification: QueryClassification) -> Vec<CoverageSlot> {
    required_slots_for(query_classification)
        .iter()
        .copied()
        .filter(|slot| matches!(slot, CoverageSlot::DecisionEvidence))
        .collect()
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

fn usize_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

fn marginal_slot_gain_per_ms(
    before: &CoverageAudit,
    after: &CoverageAudit,
    elapsed_ms: f64,
) -> f64 {
    let gained_slots = gained_slot_count(before, after);
    if elapsed_ms <= 0.0 {
        return usize_to_f64(gained_slots);
    }

    usize_to_f64(gained_slots) / elapsed_ms
}

fn gained_slot_count(before: &CoverageAudit, after: &CoverageAudit) -> usize {
    let previous_statuses = before
        .slot_results
        .iter()
        .map(|slot| (coverage_slot_label(slot.slot), slot.status))
        .collect::<BTreeMap<_, _>>();
    after
        .slot_results
        .iter()
        .filter(|slot| {
            let slot_label = coverage_slot_label(slot.slot);
            let Some(previous_status) = previous_statuses.get(slot_label).copied() else {
                return false;
            };

            !status_is_sufficient_for_audit(previous_status)
                && status_is_sufficient_for_audit(slot.status)
        })
        .count()
}

impl AdaptiveSecondPassKey {
    fn new(query_classification: QueryClassification, repo_shape: RepoShapeBucket) -> Self {
        Self {
            query_classification: query_classification_label(query_classification),
            repo_shape,
        }
    }
}

impl AdaptiveSecondPassStats {
    fn heal_ratio(&self) -> f64 {
        if self.missing_slots == 0 {
            return 0.0;
        }

        f64::from(self.healed_slots) / f64::from(self.missing_slots)
    }
}

impl AdaptiveSecondPassModel {
    fn recommend_budget(
        &self,
        query_classification: QueryClassification,
        repo_shape: RepoShapeBucket,
        base_candidates: usize,
        base_hops: u8,
        latency_class: LatencyClass,
    ) -> AdaptiveSecondPassBudget {
        let key = AdaptiveSecondPassKey::new(query_classification, repo_shape);
        let mut candidate_scale_per_thousand = match repo_shape {
            RepoShapeBucket::Tiny => 850_i32,
            RepoShapeBucket::Small => 1_000_i32,
            RepoShapeBucket::Medium => 1_150_i32,
            RepoShapeBucket::LargeDense => 1_300_i32,
        };
        let mut max_hops = match repo_shape {
            RepoShapeBucket::Tiny => base_hops.saturating_sub(1).max(1),
            RepoShapeBucket::Small => base_hops,
            RepoShapeBucket::Medium | RepoShapeBucket::LargeDense => {
                base_hops.saturating_add(1).min(4)
            }
        };
        let mut marginal_gain_threshold =
            base_marginal_gain_threshold(latency_class, query_classification);

        if let Some(stats) = self.stats.get(&key)
            && stats.observations >= 3
        {
            let heal_ratio = stats.heal_ratio();
            if heal_ratio >= 0.45 {
                candidate_scale_per_thousand = candidate_scale_per_thousand.saturating_add(200);
                max_hops = max_hops.saturating_add(1).min(4);
            } else if heal_ratio <= 0.15 || stats.gain_per_ms_ema < marginal_gain_threshold * 0.5 {
                candidate_scale_per_thousand = candidate_scale_per_thousand.saturating_sub(200);
                max_hops = max_hops.saturating_sub(1).max(1);
            }

            let adaptive_threshold = (stats.gain_per_ms_ema * 0.85).clamp(0.003, 0.08);
            marginal_gain_threshold = marginal_gain_threshold.max(adaptive_threshold);
        }

        let candidate_scale_per_thousand = candidate_scale_per_thousand.clamp(600, 1_800);
        let candidate_scale = usize::try_from(candidate_scale_per_thousand).unwrap_or(1_000);
        let max_candidates = base_candidates
            .saturating_mul(candidate_scale)
            .saturating_add(500)
            / 1_000;
        let max_candidates = max_candidates.clamp(8, 160);
        AdaptiveSecondPassBudget {
            max_candidates,
            max_hops,
            marginal_gain_threshold,
        }
    }

    fn expected_healed_slots_per_ms(
        &self,
        query_classification: QueryClassification,
        repo_shape: RepoShapeBucket,
    ) -> Option<f64> {
        let key = AdaptiveSecondPassKey::new(query_classification, repo_shape);
        self.stats.get(&key).and_then(|stats| {
            if stats.observations >= 3 {
                Some(stats.gain_per_ms_ema.max(0.0))
            } else {
                None
            }
        })
    }

    fn observe_outcome(
        &mut self,
        query_classification: QueryClassification,
        repo_shape: RepoShapeBucket,
        before: &CoverageAudit,
        after: &CoverageAudit,
        elapsed_ms: f64,
    ) {
        let key = AdaptiveSecondPassKey::new(query_classification, repo_shape);
        let stats = self.stats.entry(key).or_default();
        stats.observations = stats.observations.saturating_add(1);

        let previous_statuses = before
            .slot_results
            .iter()
            .map(|slot| (coverage_slot_label(slot.slot), slot.status))
            .collect::<BTreeMap<_, _>>();
        let mut missing_slots = 0_u32;
        let mut healed_slots = 0_u32;
        let mut unresolved_slots = 0_u32;

        for slot in &after.slot_results {
            let slot_label = coverage_slot_label(slot.slot);
            let Some(previous_status) = previous_statuses.get(slot_label).copied() else {
                continue;
            };
            let was_sufficient = status_is_sufficient_for_audit(previous_status);
            let is_sufficient = status_is_sufficient_for_audit(slot.status);

            if !was_sufficient {
                missing_slots = missing_slots.saturating_add(1);
            }
            if !was_sufficient && is_sufficient {
                healed_slots = healed_slots.saturating_add(1);
            }
            if !is_sufficient {
                unresolved_slots = unresolved_slots.saturating_add(1);
            }
        }

        stats.missing_slots = stats.missing_slots.saturating_add(missing_slots);
        stats.healed_slots = stats.healed_slots.saturating_add(healed_slots);
        stats.unresolved_slots = stats.unresolved_slots.saturating_add(unresolved_slots);

        let gain_per_ms = if elapsed_ms <= 0.0 {
            f64::from(healed_slots)
        } else {
            f64::from(healed_slots) / elapsed_ms
        };
        stats.gain_per_ms_ema = if stats.observations == 1 {
            gain_per_ms
        } else {
            (stats.gain_per_ms_ema * 0.7) + (gain_per_ms * 0.3)
        };
    }
}

impl SlotCostLedger {
    fn record_stage_cost(&mut self, slots: &[CoverageSlot], elapsed_ms: f64) {
        if slots.is_empty() || elapsed_ms <= 0.0 {
            return;
        }

        let per_slot_cost = elapsed_ms / usize_to_f64(slots.len());
        for slot in slots {
            let slot_label = coverage_slot_label(*slot);
            self.cost_ms_by_slot
                .entry(slot_label)
                .and_modify(|cost| *cost += per_slot_cost)
                .or_insert(per_slot_cost);
        }
    }

    fn record_gains_from_audits(&mut self, before: &CoverageAudit, after: &CoverageAudit) -> usize {
        let previous_statuses = before
            .slot_results
            .iter()
            .map(|slot| (coverage_slot_label(slot.slot), slot.status))
            .collect::<BTreeMap<_, _>>();
        let mut gains = 0_usize;

        for slot in &after.slot_results {
            let slot_label = coverage_slot_label(slot.slot);
            let Some(previous_status) = previous_statuses.get(slot_label).copied() else {
                continue;
            };
            if !status_is_sufficient_for_audit(previous_status)
                && status_is_sufficient_for_audit(slot.status)
            {
                self.gains_by_slot
                    .entry(slot_label)
                    .and_modify(|count| *count = count.saturating_add(1))
                    .or_insert(1);
                gains = gains.saturating_add(1);
            }
        }

        gains
    }

    fn detail_suffix(&self, slot: CoverageSlot) -> String {
        let slot_label = coverage_slot_label(slot);
        let cost_ms = self.cost_ms_by_slot.get(slot_label).copied().unwrap_or(0.0);
        let gains = self.gains_by_slot.get(slot_label).copied().unwrap_or(0);

        if cost_ms <= 0.0 && gains == 0 {
            return String::new();
        }

        format!("; retrieval_cost_ms={cost_ms:.2}; slot_gains={gains}")
    }
}

impl RetrievalFrontierController {
    fn new(floor_healed_slots_per_ms: f64, initial_forecast: Option<f64>) -> Self {
        let expected_healed_slots_per_ms = initial_forecast
            .unwrap_or(floor_healed_slots_per_ms)
            .max(0.0);
        Self {
            floor_healed_slots_per_ms,
            expected_healed_slots_per_ms,
            has_forecast: initial_forecast.is_some(),
        }
    }

    fn should_continue(&self) -> bool {
        !self.has_forecast || self.expected_healed_slots_per_ms >= self.floor_healed_slots_per_ms
    }

    fn observe(&mut self, observed_healed_slots_per_ms: f64) {
        if !observed_healed_slots_per_ms.is_finite() {
            return;
        }
        let observed = observed_healed_slots_per_ms.max(0.0);
        if self.has_forecast {
            self.expected_healed_slots_per_ms =
                (self.expected_healed_slots_per_ms * 0.7) + (observed * 0.3);
        } else {
            self.expected_healed_slots_per_ms = observed;
            self.has_forecast = true;
        }
    }

    fn expected_healed_slots_per_ms(&self) -> f64 {
        self.expected_healed_slots_per_ms
    }
}

impl AdaptiveSecondPassStore {
    fn load_stats(
        repo_root: &str,
        key: &AdaptiveSecondPassKey,
    ) -> Result<Option<AdaptiveSecondPassStats>, AdaptiveStoreError> {
        let connection = Self::open_connection(repo_root)?;
        Self::initialize_schema(&connection)?;
        connection
            .query_row(
                "SELECT observations, missing_slots, healed_slots, unresolved_slots, gain_per_ms_ema
                 FROM adaptive_second_pass_stats
                 WHERE query_classification = ?1 AND repo_shape = ?2",
                params![key.query_classification, repo_shape_code(key.repo_shape)],
                |row| {
                    Ok(AdaptiveSecondPassStats {
                        observations: saturating_i64_to_u32(row.get::<usize, i64>(0)?),
                        missing_slots: saturating_i64_to_u32(row.get::<usize, i64>(1)?),
                        healed_slots: saturating_i64_to_u32(row.get::<usize, i64>(2)?),
                        unresolved_slots: saturating_i64_to_u32(row.get::<usize, i64>(3)?),
                        gain_per_ms_ema: row.get::<usize, f64>(4)?,
                    })
                },
            )
            .optional()
            .map_err(|source| AdaptiveStoreError::Sqlite {
                operation: "load_stats",
                source,
            })
    }

    fn store_stats(
        repo_root: &str,
        key: &AdaptiveSecondPassKey,
        stats: &AdaptiveSecondPassStats,
    ) -> Result<(), AdaptiveStoreError> {
        let connection = Self::open_connection(repo_root)?;
        Self::initialize_schema(&connection)?;
        connection
            .execute(
                "INSERT INTO adaptive_second_pass_stats (
                    query_classification,
                    repo_shape,
                    observations,
                    missing_slots,
                    healed_slots,
                    unresolved_slots,
                    gain_per_ms_ema
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                 ON CONFLICT(query_classification, repo_shape) DO UPDATE SET
                    observations = excluded.observations,
                    missing_slots = excluded.missing_slots,
                    healed_slots = excluded.healed_slots,
                    unresolved_slots = excluded.unresolved_slots,
                    gain_per_ms_ema = excluded.gain_per_ms_ema",
                params![
                    key.query_classification,
                    repo_shape_code(key.repo_shape),
                    i64::from(stats.observations),
                    i64::from(stats.missing_slots),
                    i64::from(stats.healed_slots),
                    i64::from(stats.unresolved_slots),
                    stats.gain_per_ms_ema,
                ],
            )
            .map(|_| ())
            .map_err(|source| AdaptiveStoreError::Sqlite {
                operation: "store_stats",
                source,
            })
    }

    fn open_connection(repo_root: &str) -> Result<Connection, AdaptiveStoreError> {
        let path = adaptive_store_db_path(repo_root);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AdaptiveStoreError::CreateDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        Connection::open(path).map_err(|source| AdaptiveStoreError::Sqlite {
            operation: "open_connection",
            source,
        })
    }

    fn initialize_schema(connection: &Connection) -> Result<(), AdaptiveStoreError> {
        connection
            .execute_batch(
                "
                CREATE TABLE IF NOT EXISTS adaptive_second_pass_stats (
                    query_classification TEXT NOT NULL,
                    repo_shape INTEGER NOT NULL,
                    observations INTEGER NOT NULL,
                    missing_slots INTEGER NOT NULL,
                    healed_slots INTEGER NOT NULL,
                    unresolved_slots INTEGER NOT NULL,
                    gain_per_ms_ema REAL NOT NULL,
                    PRIMARY KEY (query_classification, repo_shape)
                );
                ",
            )
            .map_err(|source| AdaptiveStoreError::Sqlite {
                operation: "initialize_schema",
                source,
            })
    }
}

fn adaptive_store_db_path(repo_root: &str) -> PathBuf {
    Path::new(repo_root)
        .join(".repobrain")
        .join("indexes")
        .join("adaptive-second-pass.sqlite3")
}

fn repo_shape_code(bucket: RepoShapeBucket) -> i64 {
    match bucket {
        RepoShapeBucket::Tiny => 1,
        RepoShapeBucket::Small => 2,
        RepoShapeBucket::Medium => 3,
        RepoShapeBucket::LargeDense => 4,
    }
}

fn saturating_i64_to_u32(value: i64) -> u32 {
    if value <= 0 {
        0
    } else {
        u32::try_from(value).unwrap_or(u32::MAX)
    }
}

fn merge_lexical_hits(existing: &mut Vec<LexicalHit>, additional: &[LexicalHit], limit: usize) {
    let mut merged_scores = BTreeMap::<String, f32>::new();
    for hit in existing.iter().chain(additional) {
        merged_scores
            .entry(hit.relative_path.clone())
            .and_modify(|score| {
                if hit.score > *score {
                    *score = hit.score;
                }
            })
            .or_insert(hit.score);
    }

    let mut merged = merged_scores
        .into_iter()
        .map(|(relative_path, score)| LexicalHit {
            relative_path,
            score,
        })
        .collect::<Vec<_>>();
    merged.sort_unstable_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let effective_limit = if limit == 0 { merged.len() } else { limit };
    merged.truncate(effective_limit);
    *existing = merged;
}

fn merge_graph_neighbors(
    existing: &mut Vec<GraphNeighbor>,
    additional: &[GraphNeighbor],
    limit: usize,
) {
    let mut merged_hops = BTreeMap::<String, u8>::new();
    for neighbor in existing.iter().chain(additional) {
        merged_hops
            .entry(neighbor.relative_path.clone())
            .and_modify(|hop_distance| {
                if neighbor.hop_distance < *hop_distance {
                    *hop_distance = neighbor.hop_distance;
                }
            })
            .or_insert(neighbor.hop_distance);
    }

    let mut merged = merged_hops
        .into_iter()
        .map(|(relative_path, hop_distance)| GraphNeighbor {
            relative_path,
            hop_distance,
        })
        .collect::<Vec<_>>();
    merged.sort_unstable_by(|left, right| {
        left.hop_distance
            .cmp(&right.hop_distance)
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    let effective_limit = if limit == 0 { merged.len() } else { limit };
    merged.truncate(effective_limit);
    *existing = merged;
}

fn merge_evidence_receipts(existing: &mut Vec<EvidenceReceipt>, additional: Vec<EvidenceReceipt>) {
    existing.extend(additional);
    existing.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    existing.dedup_by(|left, right| left.id == right.id);
}

fn packing_budget(latency_class: LatencyClass, token_budget: u32) -> PackingBudget {
    let (mut brief_items, mut suggested_files) = token_packing_targets(token_budget);
    let fragile_zones = match latency_class {
        LatencyClass::Instant => {
            brief_items = brief_items.min(3);
            suggested_files = suggested_files.min(4);
            2
        }
        LatencyClass::Interactive => {
            brief_items = brief_items.min(16);
            suggested_files = suggested_files.min(24);
            3
        }
        LatencyClass::Deep => {
            brief_items = brief_items.min(48);
            suggested_files = suggested_files.min(96);
            4
        }
        LatencyClass::Background => {
            brief_items = brief_items.min(256);
            suggested_files = suggested_files.min(512);
            8
        }
    };

    PackingBudget {
        brief_items,
        suggested_files,
        fragile_zones,
    }
}

fn token_packing_targets(token_budget: u32) -> (usize, usize) {
    match token_budget {
        0..=2_048 => (3, 4),
        2_049..=4_096 => (5, 6),
        _ => {
            let four_k_chunks = token_budget.div_ceil(4_096) as usize;
            let brief_items = 5usize.saturating_add(four_k_chunks).min(256);
            let suggested_files = 4usize
                .saturating_add(four_k_chunks.saturating_mul(2))
                .min(512);
            (brief_items, suggested_files)
        }
    }
}

fn collect_suggested_files(impacted_nodes: &[String], limit: usize) -> Vec<String> {
    let mut suggested_files = Vec::new();
    let mut seen = BTreeSet::new();

    for node in impacted_nodes {
        let Some(path) = node.strip_prefix("file:") else {
            continue;
        };
        let path = path.to_string();
        if !seen.insert(path.clone()) {
            continue;
        }

        suggested_files.push(path);
        if suggested_files.len() == limit {
            break;
        }
    }

    suggested_files
}

fn merge_suggested_files(
    mut suggested_files: Vec<String>,
    lexical_hits: &[LexicalHit],
    neighbors: &[GraphNeighbor],
    limit: usize,
) -> Vec<String> {
    if suggested_files.len() >= limit {
        return suggested_files;
    }

    let mut seen = suggested_files
        .iter()
        .cloned()
        .collect::<BTreeSet<String>>();
    for hit in lexical_hits {
        if suggested_files.len() >= limit {
            break;
        }
        if seen.insert(hit.relative_path.clone()) {
            suggested_files.push(hit.relative_path.clone());
        }
    }
    for neighbor in neighbors {
        if suggested_files.len() >= limit {
            break;
        }
        if seen.insert(neighbor.relative_path.clone()) {
            suggested_files.push(neighbor.relative_path.clone());
        }
    }

    suggested_files
}

fn collect_anchor_paths(
    exact_path_hit: Option<&IndexedFile>,
    exact_symbol_hits: &[IndexedSymbol],
) -> BTreeSet<String> {
    let mut anchor_paths = BTreeSet::new();
    if let Some(file) = exact_path_hit {
        let _ = anchor_paths.insert(file.relative_path.clone());
    }

    for symbol in exact_symbol_hits {
        let _ = anchor_paths.insert(symbol.relative_path.clone());
    }

    anchor_paths
}

fn collect_anchor_paths_with_lexical(
    exact_path_hit: Option<&IndexedFile>,
    exact_symbol_hits: &[IndexedSymbol],
    lexical_hits: &[LexicalHit],
) -> BTreeSet<String> {
    let mut anchor_paths = collect_anchor_paths(exact_path_hit, exact_symbol_hits);
    for hit in lexical_hits {
        let _ = anchor_paths.insert(hit.relative_path.clone());
    }

    anchor_paths
}

fn seed_paths_for_expansion(
    exact_path_hit: Option<&IndexedFile>,
    exact_symbol_hits: &[IndexedSymbol],
    lexical_hits: &[LexicalHit],
) -> Vec<String> {
    collect_anchor_paths_with_lexical(exact_path_hit, exact_symbol_hits, lexical_hits)
        .into_iter()
        .collect()
}

const MAX_DECISION_DOC_BYTES: u64 = 256 * 1024;
const MAX_DECISION_SNIPPET_CHARS: usize = 160;
const DECISION_LEXICAL_CANDIDATE_LIMIT: usize = 64;
const DECISION_CANDIDATE_SCAN_BUDGET: usize = 24;
const DECISION_EMPTY_SCAN_STREAK_LIMIT: usize = 6;
const DECISION_SCOPE_PROBE_TERMS: usize = 2;

#[derive(Debug, Default)]
struct DecisionTermSets {
    scope: BTreeSet<String>,
    anchor: BTreeSet<String>,
    suggested: BTreeSet<String>,
}

fn decision_paths_cache_key(snapshot: &RepositoryInventorySnapshot) -> String {
    format!("{}::{}", snapshot.snapshot_id, snapshot.captured_at)
}

fn build_decision_path_baseline(snapshot: &RepositoryInventorySnapshot) -> Vec<String> {
    let mut decision_paths = snapshot
        .files
        .iter()
        .filter(|file| {
            file.size_bytes <= MAX_DECISION_DOC_BYTES
                && looks_like_decision_doc(
                    &file.relative_path,
                    file.language.as_deref(),
                    file.extension.as_deref(),
                )
        })
        .map(|file| normalize_path(&file.relative_path))
        .collect::<Vec<_>>();
    decision_paths.sort_unstable_by(|left, right| {
        decision_path_priority(right)
            .cmp(&decision_path_priority(left))
            .then_with(|| left.cmp(right))
    });

    decision_paths
}

fn collect_relevant_decisions(
    context: &DecisionEvidenceQueryContext<'_>,
    limit: usize,
) -> Vec<String> {
    let terms =
        build_decision_term_sets(context.scope, context.anchor_paths, context.suggested_files);
    let baseline_decision_paths = if context.report_decision_paths.is_empty() {
        context
            .decision_path_cache
            .paths_for_snapshot(context.snapshot)
    } else {
        let mut normalized = context
            .report_decision_paths
            .iter()
            .map(|path| normalize_path(path))
            .collect::<Vec<_>>();
        normalized.sort_unstable();
        normalized.dedup();
        normalized
    };
    let files_by_path = context
        .snapshot
        .files
        .iter()
        .map(|file| (normalize_path(&file.relative_path), file))
        .collect::<BTreeMap<_, _>>();
    let mut fast_scored = collect_fast_decision_candidates(
        context.anchor_paths,
        context.suggested_files,
        &files_by_path,
        &baseline_decision_paths,
        &terms,
    );
    if !fast_scored.is_empty() {
        fast_scored.sort_unstable();
        return fast_scored
            .into_iter()
            .map(|(_, _, rendered)| rendered)
            .take(limit)
            .collect();
    }

    let candidate_paths = collect_decision_candidate_paths(
        context.snapshot,
        context.scope,
        context.anchor_paths,
        context.suggested_files,
        context.lexical_cache,
        &terms,
    );
    let mut scored = score_decision_candidates(
        context.snapshot,
        &files_by_path,
        &candidate_paths,
        &terms,
        limit,
    );
    scored.sort_unstable();
    scored
        .into_iter()
        .map(|(_, _, rendered)| rendered)
        .take(limit)
        .collect()
}

fn collect_fast_decision_candidates(
    anchor_paths: &BTreeSet<String>,
    suggested_files: &[String],
    files_by_path: &BTreeMap<String, &IndexedFile>,
    baseline_decision_paths: &[String],
    terms: &DecisionTermSets,
) -> Vec<(usize, String, String)> {
    let mut scored_by_path = BTreeMap::<String, usize>::new();

    for path in anchor_paths.iter().chain(suggested_files.iter()) {
        let normalized = normalize_path(path);
        let Some(file) = files_by_path.get(&normalized) else {
            continue;
        };
        if !looks_like_decision_doc(
            &file.relative_path,
            file.language.as_deref(),
            file.extension.as_deref(),
        ) {
            continue;
        }
        let ranking_score =
            decision_candidate_seed_score(&file.relative_path, terms).saturating_add(4);
        record_decision_candidate_score(&mut scored_by_path, normalized, ranking_score);
    }

    for fallback_path in baseline_decision_paths
        .iter()
        .take(DECISION_LEXICAL_CANDIDATE_LIMIT)
    {
        let Some(file) = files_by_path.get(fallback_path) else {
            continue;
        };
        let ranking_score = decision_candidate_seed_score(&file.relative_path, terms);
        record_decision_candidate_score(&mut scored_by_path, fallback_path.clone(), ranking_score);
    }

    let mut scored = scored_by_path
        .into_iter()
        .filter_map(|(path, score)| {
            let file = files_by_path.get(&path)?;
            let rendered = render_fast_decision_candidate(&file.relative_path);
            Some((usize::MAX - score, file.relative_path.clone(), rendered))
        })
        .collect::<Vec<_>>();
    scored.sort_unstable();

    scored
}

fn decision_scope_probe_query(scope: &str) -> String {
    let camel_terms = camel_case_terms(scope);
    if camel_terms.len() >= 2 {
        let probe_terms = vec![
            camel_terms.first().cloned().unwrap_or_default(),
            camel_terms.last().cloned().unwrap_or_default(),
        ]
        .into_iter()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
        if !probe_terms.is_empty() {
            return probe_terms.join(" ");
        }
    }

    let mut terms = text_terms(scope)
        .into_iter()
        .filter(|term| !is_generic_probe_term(term))
        .collect::<Vec<_>>();
    terms
        .sort_unstable_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    terms.truncate(DECISION_SCOPE_PROBE_TERMS);
    terms.join(" ")
}

fn record_decision_candidate_score(
    scored_by_path: &mut BTreeMap<String, usize>,
    normalized_path: String,
    score: usize,
) {
    let current = scored_by_path.entry(normalized_path).or_insert(0);
    if score > *current {
        *current = score;
    }
}

fn render_fast_decision_candidate(relative_path: &str) -> String {
    let title = infer_decision_title_from_path(relative_path);
    format!("{title} [{relative_path}]: indexed lexical decision evidence candidate")
}

fn infer_decision_title_from_path(relative_path: &str) -> String {
    if let Some(adr_id) = adr_id_from_path(relative_path) {
        return adr_id;
    }

    Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map_or_else(
            || relative_path.to_string(),
            |stem| stem.replace(['_', '-'], " ").trim().to_string(),
        )
}

fn adr_id_from_path(relative_path: &str) -> Option<String> {
    let path_lower = relative_path.to_ascii_lowercase();
    let marker_index = path_lower.find("adr-")?;
    let digits = path_lower[marker_index + 4..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        None
    } else {
        Some(format!("ADR-{digits}"))
    }
}

fn camel_case_terms(value: &str) -> Vec<String> {
    let mut terms = Vec::new();
    for token in value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let parts = split_camel_case_token(token);
        for part in parts {
            if part.len() >= 3 && !is_generic_probe_term(&part) {
                terms.push(part);
            }
        }
    }
    terms.sort_unstable();
    terms.dedup();
    terms
}

fn split_camel_case_token(token: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();

    for character in token.chars() {
        if character.is_ascii_uppercase() && !current.is_empty() {
            parts.push(current.to_ascii_lowercase());
            current.clear();
        }
        if character.is_ascii_alphanumeric() {
            current.push(character);
        }
    }
    if !current.is_empty() {
        parts.push(current.to_ascii_lowercase());
    }

    parts
}

fn is_generic_probe_term(term: &str) -> bool {
    matches!(
        term,
        "src"
            | "rust"
            | "crates"
            | "lib"
            | "main"
            | "docs"
            | "plans"
            | "research"
            | "readme"
            | "cargo"
            | "toml"
            | "md"
            | "rs"
            | "the"
            | "this"
            | "that"
            | "why"
            | "what"
            | "when"
            | "where"
            | "with"
            | "from"
            | "into"
            | "for"
            | "and"
            | "or"
            | "not"
            | "how"
            | "used"
            | "design"
    )
}

fn build_decision_term_sets(
    scope: &str,
    anchor_paths: &BTreeSet<String>,
    suggested_files: &[String],
) -> DecisionTermSets {
    let mut scope_terms = text_terms(scope);
    for term in camel_case_terms(scope) {
        let _ = scope_terms.insert(term);
    }

    DecisionTermSets {
        scope: scope_terms,
        anchor: anchor_paths
            .iter()
            .flat_map(|path| path_terms(path))
            .collect::<BTreeSet<_>>(),
        suggested: suggested_files
            .iter()
            .flat_map(|path| path_terms(path))
            .collect::<BTreeSet<_>>(),
    }
}

fn collect_decision_candidate_paths(
    snapshot: &RepositoryInventorySnapshot,
    scope: &str,
    anchor_paths: &BTreeSet<String>,
    suggested_files: &[String],
    lexical_cache: &SnapshotLexicalIndexCache,
    terms: &DecisionTermSets,
) -> Vec<String> {
    let mut candidate_paths = Vec::<String>::new();
    let mut candidate_seen = BTreeSet::<String>::new();

    let decision_query = decision_scope_probe_query(scope);
    if let Ok(lexical_hits) = lexical_cache.search(
        snapshot,
        &LexicalQuery {
            text: decision_query,
            scope: None,
            limit: DECISION_LEXICAL_CANDIDATE_LIMIT,
        },
    ) {
        for hit in lexical_hits {
            push_unique_path_candidate(
                &hit.relative_path,
                &mut candidate_paths,
                &mut candidate_seen,
            );
        }
    }
    for path in anchor_paths.iter().chain(suggested_files.iter()) {
        push_unique_path_candidate(path, &mut candidate_paths, &mut candidate_seen);
    }

    candidate_paths.extend(collect_fallback_decision_candidates(
        snapshot,
        &candidate_seen,
        terms,
    ));

    candidate_paths
}

fn push_unique_path_candidate(
    path: &str,
    candidate_paths: &mut Vec<String>,
    candidate_seen: &mut BTreeSet<String>,
) {
    let normalized_path = normalize_path(path);
    if candidate_seen.insert(normalized_path.clone()) {
        candidate_paths.push(normalized_path);
    }
}

fn collect_fallback_decision_candidates(
    snapshot: &RepositoryInventorySnapshot,
    candidate_seen: &BTreeSet<String>,
    terms: &DecisionTermSets,
) -> Vec<String> {
    let mut fallback_paths = snapshot
        .files
        .iter()
        .filter(|file| {
            file.size_bytes <= MAX_DECISION_DOC_BYTES
                && looks_like_decision_doc(
                    &file.relative_path,
                    file.language.as_deref(),
                    file.extension.as_deref(),
                )
        })
        .map(|file| normalize_path(&file.relative_path))
        .filter(|path| !candidate_seen.contains(path))
        .filter(|path| should_keep_fallback_decision_candidate(path, terms))
        .collect::<Vec<_>>();

    fallback_paths.sort_unstable_by(|left, right| {
        decision_candidate_seed_score(right, terms)
            .cmp(&decision_candidate_seed_score(left, terms))
            .then_with(|| left.cmp(right))
    });

    fallback_paths
}

fn should_keep_fallback_decision_candidate(path: &str, terms: &DecisionTermSets) -> bool {
    let path_lower = path.to_ascii_lowercase();
    if decision_path_priority(&path_lower) >= 10 {
        return true;
    }

    path_term_overlap_count(&path_lower, &terms.scope) > 0
        || path_term_overlap_count(&path_lower, &terms.anchor) > 0
        || path_term_overlap_count(&path_lower, &terms.suggested) > 0
}

fn decision_candidate_seed_score(path: &str, terms: &DecisionTermSets) -> usize {
    let path_lower = path.to_ascii_lowercase();
    let mut score = decision_path_priority(&path_lower);
    score += path_term_overlap_count(&path_lower, &terms.scope) * 8;
    score += path_term_overlap_count(&path_lower, &terms.anchor) * 5;
    score += path_term_overlap_count(&path_lower, &terms.suggested) * 3;

    score
}

fn path_term_overlap_count(path_lower: &str, terms: &BTreeSet<String>) -> usize {
    terms
        .iter()
        .filter(|term| path_lower.contains(term.as_str()))
        .count()
}

fn score_decision_candidates(
    snapshot: &RepositoryInventorySnapshot,
    files_by_path: &BTreeMap<String, &IndexedFile>,
    candidate_paths: &[String],
    terms: &DecisionTermSets,
    limit: usize,
) -> Vec<(usize, String, String)> {
    let mut scored = Vec::<(usize, String, String)>::new();
    let mut empty_scan_streak = 0_usize;
    let target_results = decision_target_result_count(limit);
    let scan_budget = limit.max(1).saturating_mul(DECISION_CANDIDATE_SCAN_BUDGET);

    for candidate_path in candidate_paths.iter().take(scan_budget) {
        let Some(file) = files_by_path.get(candidate_path) else {
            continue;
        };
        let Some(scored_candidate) = score_decision_candidate(snapshot, file, terms) else {
            empty_scan_streak = empty_scan_streak.saturating_add(1);
            if scored.len() >= limit.max(1) && empty_scan_streak >= DECISION_EMPTY_SCAN_STREAK_LIMIT
            {
                break;
            }
            continue;
        };

        scored.push(scored_candidate);
        empty_scan_streak = 0;
        if scored.len() >= target_results {
            break;
        }
    }

    scored
}

fn decision_target_result_count(limit: usize) -> usize {
    let limit = limit.max(1);
    limit.saturating_add(limit / 2)
}

fn score_decision_candidate(
    snapshot: &RepositoryInventorySnapshot,
    file: &IndexedFile,
    terms: &DecisionTermSets,
) -> Option<(usize, String, String)> {
    if file.size_bytes > MAX_DECISION_DOC_BYTES {
        return None;
    }
    if !looks_like_decision_doc(
        &file.relative_path,
        file.language.as_deref(),
        file.extension.as_deref(),
    ) {
        return None;
    }

    let path = Path::new(&snapshot.root).join(&file.relative_path);
    let Ok(content_profile) = scan_decision_document(
        &path,
        &terms.scope,
        &terms.anchor,
        &terms.suggested,
        MAX_DECISION_SNIPPET_CHARS,
    ) else {
        return None;
    };
    if !content_profile.has_markers {
        return None;
    }

    let ranking_score = decision_doc_score(&file.relative_path, &content_profile);
    let title = content_profile
        .title
        .unwrap_or_else(|| file.relative_path.clone());
    let snippet = content_profile
        .snippet
        .unwrap_or_else(|| "decision context is documented in this file".to_string());
    let rendered = format!("{title} [{}]: {snippet}", file.relative_path);

    Some((
        usize::MAX - ranking_score,
        file.relative_path.clone(),
        rendered,
    ))
}

fn looks_like_decision_doc(path: &str, language: Option<&str>, extension: Option<&str>) -> bool {
    if let Some(language) = language
        && language == "markdown"
    {
        return true;
    }
    if let Some(extension) = extension
        && extension.eq_ignore_ascii_case("md")
    {
        return true;
    }

    path.to_ascii_lowercase().contains("adr")
        || path.to_ascii_lowercase().contains("decision")
        || path.to_ascii_lowercase().contains("intent")
        || path.to_ascii_lowercase().contains("spec")
}

fn line_contains_decision_marker(lowered_line: &str) -> bool {
    [
        "decision",
        "rationale",
        "tradeoff",
        "why",
        "intent",
        "status:",
    ]
    .iter()
    .any(|marker| lowered_line.contains(marker))
}

#[derive(Debug, Default)]
struct DecisionDocContentProfile {
    has_markers: bool,
    scope_overlap: usize,
    anchor_overlap: usize,
    suggested_overlap: usize,
    title: Option<String>,
    snippet: Option<String>,
}

fn scan_decision_document(
    path: &Path,
    scope_terms: &BTreeSet<String>,
    anchor_terms: &BTreeSet<String>,
    suggested_terms: &BTreeSet<String>,
    max_snippet_chars: usize,
) -> std::io::Result<DecisionDocContentProfile> {
    let mut profile = DecisionDocContentProfile::default();
    let mut scope_hits = BTreeSet::<String>::new();
    let mut anchor_hits = BTreeSet::<String>::new();
    let mut suggested_hits = BTreeSet::<String>::new();
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if profile.title.is_none() && trimmed.starts_with('#') && trimmed.len() > 1 {
            profile.title = Some(trimmed.to_string());
        }

        if !trimmed.is_empty() {
            let lowered = trimmed.to_ascii_lowercase();
            if line_contains_decision_marker(&lowered) {
                profile.has_markers = true;
                if profile.snippet.is_none() {
                    profile.snippet = Some(compact_snippet(trimmed, max_snippet_chars));
                }
            }

            mark_term_hits(scope_terms, &mut scope_hits, &lowered);
            mark_term_hits(anchor_terms, &mut anchor_hits, &lowered);
            mark_term_hits(suggested_terms, &mut suggested_hits, &lowered);
        }

        line.clear();
    }

    profile.scope_overlap = scope_hits.len();
    profile.anchor_overlap = anchor_hits.len();
    profile.suggested_overlap = suggested_hits.len();

    Ok(profile)
}

fn compact_snippet(value: &str, max_chars: usize) -> String {
    let compact = value.replace('\t', " ");
    let truncated = compact.chars().take(max_chars).collect::<String>();
    if compact.chars().count() <= max_chars {
        return truncated;
    }

    format!("{truncated}...")
}

fn mark_term_hits(terms: &BTreeSet<String>, hits: &mut BTreeSet<String>, lowered_line: &str) {
    for term in terms {
        if hits.contains(term) {
            continue;
        }
        if lowered_line.contains(term.as_str()) {
            let _ = hits.insert(term.clone());
        }
    }
}

fn decision_doc_score(relative_path: &str, content_profile: &DecisionDocContentProfile) -> usize {
    let path_lower = relative_path.to_ascii_lowercase();
    let mut score = decision_path_priority(&path_lower);

    score += content_profile.scope_overlap * 25;
    score += content_profile.anchor_overlap * 10;
    score += content_profile.suggested_overlap * 6;

    score
}

fn decision_path_priority(relative_path: &str) -> usize {
    let path_lower = relative_path.to_ascii_lowercase();
    let mut score = 0_usize;
    if path_lower.contains("adr") {
        score += 40;
    }
    if path_lower.contains("decision") {
        score += 25;
    }
    if path_lower.contains("intent") || path_lower.contains("spec") {
        score += 10;
    }
    if path_lower.starts_with("docs/") {
        score += 6;
    }
    if path_lower.starts_with("plans/") {
        score += 5;
    }
    if path_lower.starts_with("research/") {
        score += 4;
    }

    score
}

fn text_terms(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

fn path_terms(path: &str) -> BTreeSet<String> {
    text_terms(path)
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

struct MustKnowContext<'a> {
    scope: &'a str,
    exact_path_hit: Option<&'a IndexedFile>,
    exact_symbol_hits: &'a [IndexedSymbol],
    lexical_hits: &'a [LexicalHit],
    suggested_files: &'a [String],
    flow_capsules: &'a [FlowCapsule],
    evidence_index: &'a [EvidenceReceipt],
    verification_plan: &'a VerificationPlan,
    freshness: Freshness,
    coverage_audit: &'a CoverageAudit,
}

struct CoverageAuditContext<'a> {
    query_classification: QueryClassification,
    snapshot: &'a RepositoryInventorySnapshot,
    readiness_state: ReadinessState,
    exact_path_hit: Option<&'a IndexedFile>,
    exact_symbol_hits: &'a [IndexedSymbol],
    relationship_count: usize,
    flow_capsules: &'a [FlowCapsule],
    verification_plan: &'a VerificationPlan,
    impact_summary: Option<&'a ImpactSummary>,
    relevant_decisions: &'a [String],
    suggested_files: &'a [String],
    anchor_path_count: usize,
    lexical_anchor_count: usize,
    slot_cost_ledger: Option<&'a SlotCostLedger>,
}

#[allow(clippy::too_many_lines)]
fn collect_must_know(context: &MustKnowContext<'_>) -> Vec<BriefingItem> {
    let mut items = Vec::new();

    if let Some(file) = context.exact_path_hit {
        let language = file.language.as_deref().unwrap_or("unknown");
        items.push(BriefingItem {
            statement: format!(
                "Exact path `{}` is indexed as `{language}` at `{}` ({} bytes).",
                context.scope, file.relative_path, file.size_bytes
            ),
            confidence: 0.99,
            freshness: context.freshness,
            evidence_ids: matching_receipt_ids(
                context.evidence_index,
                "file_anchor",
                &file.relative_path,
                None,
            ),
        });
    }

    for symbol in context.exact_symbol_hits.iter().take(2) {
        items.push(BriefingItem {
            statement: format!(
                "Exact symbol `{}` is defined in `{}`:{} as a `{}`.",
                symbol.name,
                symbol.relative_path,
                symbol.line_number,
                symbol_kind_label(symbol.kind)
            ),
            confidence: 0.97,
            freshness: context.freshness,
            evidence_ids: matching_receipt_ids(
                context.evidence_index,
                "symbol_definition",
                &symbol.fact_id,
                None,
            ),
        });
    }

    if context.exact_path_hit.is_none()
        && context.exact_symbol_hits.is_empty()
        && !context.lexical_hits.is_empty()
    {
        let matched = context
            .lexical_hits
            .iter()
            .take(4)
            .map(|hit| hit.relative_path.clone())
            .collect::<Vec<_>>();
        items.push(BriefingItem {
            statement: format!(
                "Lexical anchors for `{}` matched {} path(s): {}.",
                context.scope,
                context.lexical_hits.len(),
                matched.join(", ")
            ),
            confidence: 0.7,
            freshness: context.freshness,
            evidence_ids: matched
                .iter()
                .flat_map(|path| {
                    matching_receipt_ids(context.evidence_index, "lexical_hit", path, None)
                })
                .collect(),
        });
    }

    if context.suggested_files.len() > 1 {
        let structural_evidence_ids = context
            .flow_capsules
            .first()
            .map(|capsule| {
                capsule
                    .evidence
                    .iter()
                    .map(|receipt| receipt.id.clone())
                    .collect()
            })
            .unwrap_or_default();
        items.push(BriefingItem {
            statement: format!(
                "Direct structural adjacency from `{scope}` reaches {} indexed file(s): {}.",
                context.suggested_files.len(),
                summarize_paths(context.suggested_files, 4),
                scope = context.scope
            ),
            confidence: 0.82,
            freshness: context.freshness,
            evidence_ids: structural_evidence_ids,
        });
    }

    if !context.verification_plan.required_checks.is_empty() {
        items.push(BriefingItem {
            statement: format!(
                "Deterministic required checks are known for this scope: {}.",
                context.verification_plan.required_checks.join(", ")
            ),
            confidence: 0.9,
            freshness: context.freshness,
            evidence_ids: receipt_ids_for_checks(
                context.evidence_index,
                &context.verification_plan.required_checks,
            ),
        });
    }

    if !context.coverage_audit.sufficient {
        items.push(BriefingItem {
            statement: context.coverage_audit.summary.clone(),
            confidence: 0.72,
            freshness: Freshness::Unverified,
            evidence_ids: Vec::new(),
        });
    }

    if !context.verification_plan.coverage_gaps.is_empty() {
        items.push(BriefingItem {
            statement: context.verification_plan.coverage_gaps[0].clone(),
            confidence: 0.88,
            freshness: Freshness::Unverified,
            evidence_ids: Vec::new(),
        });
    }

    items
}

fn collect_fragile_zones(
    suggested_files: &[String],
    anchor_paths: &BTreeSet<String>,
    coverage_gaps: &[String],
    limit: usize,
) -> Vec<String> {
    let mut fragile_zones = Vec::new();

    for path in suggested_files {
        if anchor_paths.contains(path) {
            continue;
        }

        fragile_zones.push(format!(
            "direct structural neighbor `{path}` may require coordinated review"
        ));
        if fragile_zones.len() == limit {
            return fragile_zones;
        }
    }

    for gap in coverage_gaps {
        fragile_zones.push(gap.clone());
        if fragile_zones.len() == limit {
            break;
        }
    }

    fragile_zones
}

fn collect_relevant_flows(flow_capsules: &[FlowCapsule]) -> Vec<String> {
    flow_capsules
        .iter()
        .map(render_flow_capsule)
        .collect::<Vec<_>>()
}

fn classify_request(request: &ContextRequest) -> QueryClassification {
    let text = format!("{} {}", request.goal, request.question).to_ascii_lowercase();

    match request.task_type {
        TaskType::RepoOnboarding | TaskType::ExplainFlow => {
            QueryClassification::ArchitectureExplanation
        }
        TaskType::SafeEdit | TaskType::BlastRadius | TaskType::SemanticDiff => {
            if text_contains_any(
                &text,
                &[
                    "bug",
                    "fix",
                    "broken",
                    "regression",
                    "error",
                    "failing",
                    "failure",
                ],
            ) {
                QueryClassification::BugFix
            } else if text_contains_any(
                &text,
                &[
                    "feature",
                    "implement",
                    "add ",
                    "support ",
                    "introduce",
                    "build ",
                ],
            ) {
                QueryClassification::FeatureImplementation
            } else {
                QueryClassification::SafeEdit
            }
        }
        TaskType::Query => {
            if text_contains_any(
                &text,
                &["why", "reason", "rationale", "decision", "tradeoff"],
            ) {
                QueryClassification::DecisionWhy
            } else if text_contains_any(
                &text,
                &[
                    "architecture",
                    "flow",
                    "how does",
                    "how do",
                    "overview",
                    "explain",
                ],
            ) {
                QueryClassification::ArchitectureExplanation
            } else {
                QueryClassification::EntityLookup
            }
        }
    }
}

fn build_coverage_audit(context: &CoverageAuditContext<'_>) -> CoverageAudit {
    let required_slots = required_slots_for(context.query_classification);
    let slot_results = required_slots
        .iter()
        .copied()
        .map(|slot| audit_slot(slot, context))
        .collect::<Vec<_>>();
    let sufficient = slot_results
        .iter()
        .all(|result| status_is_sufficient_for_audit(result.status));
    let incomplete_slots = slot_results
        .iter()
        .filter(|result| !status_is_sufficient_for_audit(result.status))
        .map(|result| coverage_slot_label(result.slot))
        .collect::<Vec<_>>();
    let summary = if sufficient {
        format!(
            "Coverage audit for `{}` is sufficient across required slots: {}.",
            query_classification_label(context.query_classification),
            required_slots
                .iter()
                .map(|slot| coverage_slot_label(*slot))
                .collect::<Vec<_>>()
                .join(", ")
        )
    } else {
        format!(
            "Coverage audit for `{}` is incomplete; unresolved slots: {}.",
            query_classification_label(context.query_classification),
            incomplete_slots.join(", ")
        )
    };

    CoverageAudit {
        query_classification: context.query_classification,
        required_slots: required_slots.to_vec(),
        slot_results,
        sufficient,
        summary,
    }
}

fn required_slots_for(query_classification: QueryClassification) -> &'static [CoverageSlot] {
    match query_classification {
        QueryClassification::EntityLookup => &[CoverageSlot::ExactAnchor],
        QueryClassification::ArchitectureExplanation => &[
            CoverageSlot::ExactAnchor,
            CoverageSlot::StructuralContext,
            CoverageSlot::FlowSummary,
        ],
        QueryClassification::SafeEdit => &[
            CoverageSlot::ExactAnchor,
            CoverageSlot::StructuralContext,
            CoverageSlot::VerificationTargets,
        ],
        QueryClassification::BugFix => &[
            CoverageSlot::ExactAnchor,
            CoverageSlot::StructuralContext,
            CoverageSlot::VerificationTargets,
            CoverageSlot::ImpactEnvelope,
        ],
        QueryClassification::FeatureImplementation => &[
            CoverageSlot::ExactAnchor,
            CoverageSlot::StructuralContext,
            CoverageSlot::FlowSummary,
            CoverageSlot::VerificationTargets,
            CoverageSlot::ImpactEnvelope,
        ],
        QueryClassification::DecisionWhy => {
            &[CoverageSlot::ExactAnchor, CoverageSlot::DecisionEvidence]
        }
    }
}

fn audit_slot(slot: CoverageSlot, context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    match slot {
        CoverageSlot::ExactAnchor => audit_exact_anchor(context),
        CoverageSlot::StructuralContext => audit_structural_context(context),
        CoverageSlot::FlowSummary => audit_flow_summary(context),
        CoverageSlot::VerificationTargets => audit_verification_targets(context),
        CoverageSlot::ImpactEnvelope => audit_impact_envelope(context),
        CoverageSlot::DecisionEvidence => audit_decision_evidence(context),
    }
}

fn audit_exact_anchor(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let path_hits = usize::from(context.exact_path_hit.is_some());
    let symbol_hits = context.exact_symbol_hits.len();
    let lexical_hits = context.lexical_anchor_count;
    let status = if path_hits + symbol_hits + lexical_hits > 0 {
        present_or_stale(context.readiness_state)
    } else {
        CoverageStatus::MissingRetrievable
    };

    CoverageSlotAudit {
        slot: CoverageSlot::ExactAnchor,
        status,
        detail: append_slot_cost_suffix(
            CoverageSlot::ExactAnchor,
            format!(
                "anchoring found {path_hits} path hit(s), {symbol_hits} symbol hit(s), and {lexical_hits} lexical hit(s)"
            ),
            context.slot_cost_ledger,
        ),
    }
}

fn audit_structural_context(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let non_anchor_files = context
        .suggested_files
        .len()
        .saturating_sub(context.anchor_path_count.min(context.suggested_files.len()));
    let (status, detail) = if non_anchor_files > 0 || context.relationship_count > 0 {
        (
            present_or_stale(context.readiness_state),
            format!(
                "structural context covers {non_anchor_files} non-anchor file(s) and {} relationship(s)",
                context.relationship_count
            ),
        )
    } else if !context.suggested_files.is_empty() {
        (
            CoverageStatus::NotObservable,
            "exact anchor is known, but no non-anchor structural neighbors were found".to_string(),
        )
    } else {
        (
            CoverageStatus::MissingRetrievable,
            "no structural context was assembled for this scope".to_string(),
        )
    };

    CoverageSlotAudit {
        slot: CoverageSlot::StructuralContext,
        status,
        detail: append_slot_cost_suffix(
            CoverageSlot::StructuralContext,
            detail,
            context.slot_cost_ledger,
        ),
    }
}

fn audit_flow_summary(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    CoverageSlotAudit {
        slot: CoverageSlot::FlowSummary,
        status: if context.flow_capsules.is_empty() {
            CoverageStatus::MissingRetrievable
        } else {
            present_or_stale(context.readiness_state)
        },
        detail: append_slot_cost_suffix(
            CoverageSlot::FlowSummary,
            format!(
                "{} flow capsule(s) are available for this scope",
                context.flow_capsules.len()
            ),
            context.slot_cost_ledger,
        ),
    }
}

fn audit_verification_targets(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let required_count = context.verification_plan.required_checks.len();
    let recommended_count = context.verification_plan.recommended_checks.len();
    let status = if required_count > 0 {
        present_or_stale(context.readiness_state)
    } else if recommended_count > 0 || context.snapshot.verification_targets().is_empty() {
        CoverageStatus::NotObservable
    } else {
        CoverageStatus::MissingRetrievable
    };

    CoverageSlotAudit {
        slot: CoverageSlot::VerificationTargets,
        status,
        detail: append_slot_cost_suffix(
            CoverageSlot::VerificationTargets,
            format!(
                "verification planning found {required_count} required and {recommended_count} recommended check(s)"
            ),
            context.slot_cost_ledger,
        ),
    }
}

fn audit_impact_envelope(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let Some(impact_summary) = context.impact_summary else {
        return CoverageSlotAudit {
            slot: CoverageSlot::ImpactEnvelope,
            status: CoverageStatus::MissingRetrievable,
            detail: append_slot_cost_suffix(
                CoverageSlot::ImpactEnvelope,
                "no impact summary was compiled for this scope".to_string(),
                context.slot_cost_ledger,
            ),
        };
    };

    CoverageSlotAudit {
        slot: CoverageSlot::ImpactEnvelope,
        status: if impact_summary.changed_scope.is_empty() {
            CoverageStatus::NotObservable
        } else {
            present_or_stale(context.readiness_state)
        },
        detail: append_slot_cost_suffix(
            CoverageSlot::ImpactEnvelope,
            format!(
                "impact envelope covers {} changed scope file(s)",
                impact_summary.changed_scope.len()
            ),
            context.slot_cost_ledger,
        ),
    }
}

fn audit_decision_evidence(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    CoverageSlotAudit {
        slot: CoverageSlot::DecisionEvidence,
        status: if context.relevant_decisions.is_empty() {
            if snapshot_has_decision_docs(context.snapshot) {
                CoverageStatus::MissingRetrievable
            } else {
                CoverageStatus::NotObservable
            }
        } else {
            present_or_stale(context.readiness_state)
        },
        detail: append_slot_cost_suffix(
            CoverageSlot::DecisionEvidence,
            if context.relevant_decisions.is_empty() {
                "no deterministic decision evidence is available in the current slice".to_string()
            } else {
                format!(
                    "{} decision evidence item(s) are available for this scope",
                    context.relevant_decisions.len()
                )
            },
            context.slot_cost_ledger,
        ),
    }
}

fn append_slot_cost_suffix(
    slot: CoverageSlot,
    detail: String,
    slot_cost_ledger: Option<&SlotCostLedger>,
) -> String {
    let Some(slot_cost_ledger) = slot_cost_ledger else {
        return detail;
    };
    let suffix = slot_cost_ledger.detail_suffix(slot);
    if suffix.is_empty() {
        detail
    } else {
        format!("{detail}{suffix}")
    }
}

fn status_is_sufficient_for_audit(status: CoverageStatus) -> bool {
    matches!(
        status,
        CoverageStatus::Present | CoverageStatus::NotApplicable
    )
}

fn present_or_stale(readiness_state: ReadinessState) -> CoverageStatus {
    match readiness_state {
        ReadinessState::Stale | ReadinessState::BulkRefresh => CoverageStatus::Stale,
        _ => CoverageStatus::Present,
    }
}

fn snapshot_has_decision_docs(snapshot: &RepositoryInventorySnapshot) -> bool {
    snapshot.files.iter().any(|file| {
        looks_like_decision_doc(
            &file.relative_path,
            file.language.as_deref(),
            file.extension.as_deref(),
        )
    })
}

fn build_impact_summary(scope: &str, suggested_files: &[String]) -> Option<ImpactSummary> {
    if suggested_files.is_empty() {
        return None;
    }

    Some(ImpactSummary {
        reference: scope.to_string(),
        summary: format!(
            "Snapshot-backed exact and structural lookup suggests {} indexed file(s) are relevant to `{scope}`.",
            suggested_files.len()
        ),
        changed_scope: suggested_files.to_vec(),
        stale_concepts: Vec::new(),
        freshness_impact: freshness_impact_for(suggested_files.len()),
    })
}

fn freshness_for(readiness_state: ReadinessState) -> Freshness {
    match readiness_state {
        ReadinessState::Ready => Freshness::Fresh,
        ReadinessState::Stale => Freshness::Stale,
        ReadinessState::Cold
        | ReadinessState::Warming
        | ReadinessState::Degraded
        | ReadinessState::BulkRefresh
        | ReadinessState::OverlayOnly => Freshness::Unverified,
    }
}

fn freshness_impact_for(file_count: usize) -> FreshnessImpact {
    match file_count {
        0 | 1 => FreshnessImpact::None,
        2..=4 => FreshnessImpact::Localized,
        5..=8 => FreshnessImpact::Moderate,
        _ => FreshnessImpact::High,
    }
}

fn summarize_paths(paths: &[String], limit: usize) -> String {
    let mut selected = paths.iter().take(limit).cloned().collect::<Vec<_>>();
    if paths.len() > limit {
        selected.push(format!("and {} more", paths.len() - limit));
    }

    selected.join(", ")
}

fn collect_evidence_index(
    evidence: &[EvidenceReceipt],
    flow_capsules: &[FlowCapsule],
) -> Vec<EvidenceReceipt> {
    let mut index = evidence.to_vec();
    for capsule in flow_capsules {
        index.extend(capsule.evidence.clone());
    }

    index.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    index.dedup_by(|left, right| left.id == right.id);
    index
}

fn merge_evidence_index(
    mut index: Vec<EvidenceReceipt>,
    lexical_receipts: &[EvidenceReceipt],
    expansion_receipts: &[EvidenceReceipt],
) -> Vec<EvidenceReceipt> {
    index.extend(lexical_receipts.iter().cloned());
    index.extend(expansion_receipts.iter().cloned());
    index.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    index.dedup_by(|left, right| left.id == right.id);
    index
}

fn lexical_hits_for_evidence(hits: &[LexicalHit], captured_at: &str) -> Vec<EvidenceReceipt> {
    hits.iter()
        .map(|hit| EvidenceReceipt {
            id: format!("ev_lexical_hit_{}", sanitize_for_id(&hit.relative_path)),
            source_type: "lexical_hit".to_string(),
            source_ref: hit.relative_path.clone(),
            locator: "path".to_string(),
            snippet_hash: None,
            captured_at: captured_at.to_string(),
        })
        .collect()
}

fn sanitize_for_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn render_flow_capsule(capsule: &FlowCapsule) -> String {
    let entrypoints = capsule.entrypoints.join(", ");
    let core_modules = summarize_paths(&capsule.core_modules, 4);
    if capsule.state_transitions.is_empty() {
        return format!(
            "{}: {entrypoints}; core modules {core_modules}",
            capsule.name
        );
    }

    format!(
        "{}: {entrypoints}; {}",
        capsule.name,
        summarize_paths(&capsule.state_transitions, 2)
    )
}

fn matching_receipt_ids(
    evidence_index: &[EvidenceReceipt],
    source_type: &str,
    source_ref: &str,
    locator_fragment: Option<&str>,
) -> Vec<String> {
    evidence_index
        .iter()
        .filter(|receipt| receipt.source_type == source_type)
        .filter(|receipt| source_ref.is_empty() || receipt.source_ref == source_ref)
        .filter(|receipt| {
            locator_fragment.is_none_or(|fragment| receipt.locator.contains(fragment))
        })
        .map(|receipt| receipt.id.clone())
        .collect()
}

fn receipt_ids_for_checks(evidence_index: &[EvidenceReceipt], checks: &[String]) -> Vec<String> {
    let checks_set = checks
        .iter()
        .map(|check| check.trim())
        .filter(|check| !check.is_empty())
        .collect::<std::collections::HashSet<_>>();

    let mut seen_ids = BTreeSet::new();
    evidence_index
        .iter()
        .filter(|receipt| receipt.source_type == "verification_target")
        .filter(|receipt| checks_set.contains(receipt.source_ref.trim()))
        .map(|receipt| receipt.id.clone())
        .filter(|id| seen_ids.insert(id.clone()))
        .collect()
}

fn text_contains_any(text: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|keyword| text.contains(keyword))
}

fn query_classification_label(classification: QueryClassification) -> &'static str {
    match classification {
        QueryClassification::EntityLookup => "entity_lookup",
        QueryClassification::ArchitectureExplanation => "architecture_explanation",
        QueryClassification::SafeEdit => "safe_edit",
        QueryClassification::BugFix => "bug_fix",
        QueryClassification::FeatureImplementation => "feature_implementation",
        QueryClassification::DecisionWhy => "decision_why",
    }
}

fn coverage_slot_label(slot: CoverageSlot) -> &'static str {
    match slot {
        CoverageSlot::ExactAnchor => "exact_anchor",
        CoverageSlot::StructuralContext => "structural_context",
        CoverageSlot::FlowSummary => "flow_summary",
        CoverageSlot::VerificationTargets => "verification_targets",
        CoverageSlot::ImpactEnvelope => "impact_envelope",
        CoverageSlot::DecisionEvidence => "decision_evidence",
    }
}

fn symbol_kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Constant => "constant",
        SymbolKind::Module => "module",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use repobrain_domain::{
        ConsumerType, ContextRequest, CoverageAudit, CoverageSlot, CoverageSlotAudit,
        CoverageStatus, FreshnessRequirement, LatencyClass, ModelClass, ModelProfile,
        OverlayClaimScope, OverlayKind, QueryClassification, ReadinessState, RequestDepth,
        ScaffoldingLevel, TaskType,
    };
    use repobrain_ingest::{RepositoryScanner, RepositoryTarget};
    use repobrain_serving::{FreshnessStatus, ReadinessAssessment, ServingHealth, SnapshotStatus};

    use super::{
        AdaptiveSecondPassKey, AdaptiveSecondPassModel, AdaptiveSecondPassStats,
        AdaptiveSecondPassStore, BrokerError, RepoShapeBucket, RetrievalFrontierController,
        SnapshotBrokerInput, SnapshotContextBroker, adaptive_store_db_path,
        base_marginal_gain_threshold, class_specific_frontier_floor,
        load_adaptive_second_pass_model, marginal_slot_gain_per_ms, packing_budget,
        persist_adaptive_second_pass_stats, second_pass_subqueries,
    };

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "repobrain-broker-{label}-{}-{}",
                process::id(),
                unique_suffix()
            ));

            if let Err(error) = fs::create_dir_all(&path) {
                panic!("failed to create temp repo {}: {error}", path.display());
            }

            Self { path }
        }

        fn root(&self) -> &Path {
            &self.path
        }

        fn root_str(&self) -> String {
            self.path.to_string_lossy().into_owned()
        }

        fn write_file(&self, relative_path: &str, contents: &[u8]) {
            let path = self.path.join(relative_path);
            if let Some(parent) = path.parent()
                && let Err(error) = fs::create_dir_all(parent)
            {
                panic!("failed to create parent {}: {error}", parent.display());
            }

            if let Err(error) = fs::write(&path, contents) {
                panic!("failed to write {}: {error}", path.display());
            }
        }
    }

    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn get_brief_compiles_snapshot_backed_safe_edit_guidance() {
        let repo = TempRepo::new("get-brief");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");
        repo.write_file("README.md", b"RepositoryScanner notes\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let pack = broker
            .get_brief(SnapshotBrokerInput {
                request: request_for_scope("RepositoryScanner", 4_096),
                snapshot: &snapshot,
                readiness_assessment: ready_assessment(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })
            .unwrap_or_else(|error| panic!("get_brief failed: {error}"));
        let repository_scanner_fact_id = snapshot.find_exact_symbol("RepositoryScanner")[0]
            .fact_id
            .clone();

        assert_eq!(pack.readiness_state, ReadinessState::Ready);
        assert_eq!(pack.query_classification, QueryClassification::SafeEdit);
        assert!(
            pack.must_know
                .iter()
                .any(|item| item.statement.contains("Exact symbol `RepositoryScanner`"))
        );
        assert_eq!(
            pack.suggested_files,
            vec!["src/inner.rs".to_string(), "src/lib.rs".to_string()]
        );
        assert_eq!(
            pack.verification_plan.required_checks,
            vec!["cargo test".to_string()]
        );
        assert_eq!(pack.do_not_break, pack.verification_plan.invariants);
        assert!(!pack.do_not_break.is_empty());
        assert!(
            pack.do_not_break
                .iter()
                .any(|invariant| invariant.contains("symbol `RepositoryScanner` remains defined"))
        );
        assert_eq!(
            pack.verification_targets,
            vec!["cargo test".to_string(), "cargo check".to_string()]
        );
        assert!(pack.impact_summary.is_some());
        assert_eq!(pack.relevant_flows.len(), 1);
        assert!(pack.relevant_flows[0].contains("structural flow around `RepositoryScanner`"));
        assert!(pack.coverage_audit.sufficient);
        assert_eq!(
            pack.coverage_audit.required_slots,
            vec![
                CoverageSlot::ExactAnchor,
                CoverageSlot::StructuralContext,
                CoverageSlot::VerificationTargets,
            ]
        );
        assert!(!pack.evidence_index.is_empty());
        assert!(pack.evidence_index.iter().any(|receipt| {
            receipt.source_type == "symbol_definition"
                && receipt.source_ref == repository_scanner_fact_id
        }));
        assert!(
            pack.must_know
                .iter()
                .all(|item| !item.evidence_ids.is_empty())
        );
    }

    #[test]
    fn get_brief_caps_output_for_small_token_budgets() {
        let repo = TempRepo::new("token-budget");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"mod a;\nmod b;\nmod c;\nmod d;\nmod e;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/a.rs", b"pub fn a() {}\n");
        repo.write_file("src/b.rs", b"pub fn b() {}\n");
        repo.write_file("src/c.rs", b"pub fn c() {}\n");
        repo.write_file("src/d.rs", b"pub fn d() {}\n");
        repo.write_file("src/e.rs", b"pub fn e() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let pack = broker
            .get_brief(SnapshotBrokerInput {
                request: request_for_scope("RepositoryScanner", 1_024),
                snapshot: &snapshot,
                readiness_assessment: ready_assessment(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })
            .unwrap_or_else(|error| panic!("get_brief failed: {error}"));

        assert!(pack.must_know.len() <= 3);
        assert!(pack.suggested_files.len() <= 4);
    }

    #[test]
    fn packing_budget_scales_for_large_token_windows() {
        let baseline = packing_budget(LatencyClass::Background, 4_096);
        let large = packing_budget(LatencyClass::Background, 64_000);
        let huge = packing_budget(LatencyClass::Background, 2_000_000);

        assert!(large.brief_items > baseline.brief_items);
        assert!(large.suggested_files > baseline.suggested_files);
        assert!(huge.brief_items >= 128);
        assert!(huge.suggested_files >= 256);
    }

    #[test]
    fn packing_budget_background_is_not_tighter_than_interactive() {
        for budget in [1_024, 4_096, 64_000, 2_000_000] {
            let interactive = packing_budget(LatencyClass::Interactive, budget);
            let background = packing_budget(LatencyClass::Background, budget);

            assert!(background.brief_items >= interactive.brief_items);
            assert!(background.suggested_files >= interactive.suggested_files);
            assert!(background.fragile_zones >= interactive.fragile_zones);
        }
    }

    #[test]
    fn get_brief_rejects_missing_scope_hint() {
        let repo = TempRepo::new("missing-scope");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let result = broker.get_brief(SnapshotBrokerInput {
            request: ContextRequest {
                scope_hint: None,
                ..request_for_scope("RepositoryScanner", 4_096)
            },
            snapshot: &snapshot,
            readiness_assessment: ready_assessment(),
            overlay_kind: OverlayKind::None,
            claim_scope: OverlayClaimScope::SnapshotConfirmed,
            overlay_hash: None,
            touched_paths: Vec::new(),
        });

        let Err(error) = result else {
            panic!("missing scope should fail");
        };

        assert_eq!(error, BrokerError::MissingScopeHint);
    }

    #[test]
    fn get_brief_collects_decision_evidence_when_markdown_support_exists() {
        let repo = TempRepo::new("decision-evidence");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");
        repo.write_file(
            "docs/adr-0001-repository-scanner.md",
            b"# ADR-0001: Repository Scanner Baseline\nStatus: accepted\nDecision: keep RepositoryScanner deterministic to preserve stable inventory results.\nRationale: deterministic scans keep safe-edit guidance predictable.\n",
        );

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let pack = broker
            .get_brief(SnapshotBrokerInput {
                request: ContextRequest {
                    goal: "why is RepositoryScanner deterministic?".to_string(),
                    task_type: TaskType::Query,
                    question: "why is RepositoryScanner deterministic?".to_string(),
                    ..request_for_scope("RepositoryScanner", 4_096)
                },
                snapshot: &snapshot,
                readiness_assessment: ready_assessment(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })
            .unwrap_or_else(|error| panic!("get_brief failed: {error}"));

        assert_eq!(pack.query_classification, QueryClassification::DecisionWhy);
        assert!(pack.coverage_audit.sufficient);
        assert!(!pack.relevant_decisions.is_empty());
        assert!(pack.relevant_decisions[0].contains("ADR-0001"));
    }

    #[test]
    fn get_brief_marks_decision_queries_as_incomplete_when_no_decision_evidence_exists() {
        let repo = TempRepo::new("decision-coverage");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let pack = broker
            .get_brief(SnapshotBrokerInput {
                request: ContextRequest {
                    goal: "why is RepositoryScanner designed this way?".to_string(),
                    task_type: TaskType::Query,
                    question: "why is RepositoryScanner designed this way?".to_string(),
                    ..request_for_scope("RepositoryScanner", 4_096)
                },
                snapshot: &snapshot,
                readiness_assessment: ready_assessment(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })
            .unwrap_or_else(|error| panic!("get_brief failed: {error}"));

        assert_eq!(pack.query_classification, QueryClassification::DecisionWhy);
        assert!(!pack.coverage_audit.sufficient);
        assert_eq!(
            pack.coverage_audit.required_slots,
            vec![CoverageSlot::ExactAnchor, CoverageSlot::DecisionEvidence]
        );
        assert!(
            pack.coverage_audit
                .slot_results
                .iter()
                .any(|slot| slot.slot == CoverageSlot::DecisionEvidence)
        );
        assert!(pack.relevant_decisions.is_empty());
        assert!(pack.must_know.iter().any(|item| {
            item.statement
                .contains("Coverage audit for `decision_why` is incomplete")
        }));
        assert!(
            pack.reasoning_scaffold
                .iter()
                .any(|item| item.contains("Coverage audit for `decision_why` is incomplete"))
        );
    }

    #[test]
    fn deep_decision_queries_halt_second_pass_when_marginal_gain_is_too_low() {
        let repo = TempRepo::new("decision-second-pass-stop");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let broker = SnapshotContextBroker::default();
        let pack = broker
            .get_brief(SnapshotBrokerInput {
                request: ContextRequest {
                    goal: "why is RepositoryScanner designed this way?".to_string(),
                    task_type: TaskType::Query,
                    question: "why is RepositoryScanner designed this way?".to_string(),
                    depth: RequestDepth::Deep,
                    latency_budget: Some(1_200),
                    ..request_for_scope("RepositoryScanner", 4_096)
                },
                snapshot: &snapshot,
                readiness_assessment: ready_assessment(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })
            .unwrap_or_else(|error| panic!("get_brief failed: {error}"));

        assert!(!pack.coverage_audit.sufficient);
        assert!(
            pack.coverage_audit
                .slot_results
                .iter()
                .any(|slot| slot.detail.contains("retrieval_cost_ms="))
        );
    }

    #[test]
    fn adaptive_second_pass_budget_scales_with_repo_shape_and_observations() {
        let mut model = AdaptiveSecondPassModel::default();
        let tiny_budget = model.recommend_budget(
            QueryClassification::SafeEdit,
            RepoShapeBucket::Tiny,
            32,
            2,
            LatencyClass::Interactive,
        );
        let large_budget = model.recommend_budget(
            QueryClassification::SafeEdit,
            RepoShapeBucket::LargeDense,
            32,
            2,
            LatencyClass::Interactive,
        );
        assert!(large_budget.max_candidates > tiny_budget.max_candidates);
        assert!(large_budget.max_hops >= tiny_budget.max_hops);

        let before = CoverageAudit {
            query_classification: QueryClassification::SafeEdit,
            required_slots: vec![
                CoverageSlot::ExactAnchor,
                CoverageSlot::StructuralContext,
                CoverageSlot::VerificationTargets,
            ],
            slot_results: vec![
                CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "anchor".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::StructuralContext,
                    status: CoverageStatus::MissingRetrievable,
                    detail: "structural missing".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::VerificationTargets,
                    status: CoverageStatus::NotObservable,
                    detail: "verification unavailable".to_string(),
                },
            ],
            sufficient: false,
            summary: "before".to_string(),
        };
        let after = CoverageAudit {
            query_classification: QueryClassification::SafeEdit,
            required_slots: vec![
                CoverageSlot::ExactAnchor,
                CoverageSlot::StructuralContext,
                CoverageSlot::VerificationTargets,
            ],
            slot_results: vec![
                CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "anchor".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::StructuralContext,
                    status: CoverageStatus::Present,
                    detail: "structural healed".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::VerificationTargets,
                    status: CoverageStatus::NotObservable,
                    detail: "verification unavailable".to_string(),
                },
            ],
            sufficient: false,
            summary: "after".to_string(),
        };

        for _ in 0..4 {
            model.observe_outcome(
                QueryClassification::SafeEdit,
                RepoShapeBucket::LargeDense,
                &before,
                &after,
                25.0,
            );
        }

        let adapted_budget = model.recommend_budget(
            QueryClassification::SafeEdit,
            RepoShapeBucket::LargeDense,
            32,
            2,
            LatencyClass::Interactive,
        );
        assert!(adapted_budget.max_candidates >= large_budget.max_candidates);
        assert!(adapted_budget.max_hops >= large_budget.max_hops);
        assert!(adapted_budget.marginal_gain_threshold >= 0.003);
    }

    #[test]
    fn adaptive_second_pass_stats_persist_to_sqlite() {
        let repo = TempRepo::new("adaptive-store");
        let root = repo.root_str();
        let key =
            AdaptiveSecondPassKey::new(QueryClassification::SafeEdit, RepoShapeBucket::LargeDense);
        let expected = AdaptiveSecondPassStats {
            observations: 11,
            missing_slots: 23,
            healed_slots: 9,
            unresolved_slots: 14,
            gain_per_ms_ema: 0.0215,
        };

        AdaptiveSecondPassStore::store_stats(root.as_str(), &key, &expected)
            .unwrap_or_else(|error| panic!("store failed: {error}"));
        let loaded = AdaptiveSecondPassStore::load_stats(root.as_str(), &key)
            .unwrap_or_else(|error| panic!("load failed: {error}"))
            .unwrap_or_else(|| panic!("missing persisted adaptive stats"));

        assert_eq!(loaded, expected);
        assert!(adaptive_store_db_path(root.as_str()).exists());
    }

    #[test]
    fn adaptive_store_load_failure_falls_back_to_default_model() {
        let repo = TempRepo::new("adaptive-store-load-fallback");
        repo.write_file(".repobrain", b"occupied");
        let root = repo.root_str();
        let key =
            AdaptiveSecondPassKey::new(QueryClassification::SafeEdit, RepoShapeBucket::LargeDense);

        let model = load_adaptive_second_pass_model(root.as_str(), &key);
        assert!(model.stats.is_empty());
    }

    #[test]
    fn adaptive_store_write_failure_is_best_effort() {
        let repo = TempRepo::new("adaptive-store-write-fallback");
        repo.write_file(".repobrain", b"occupied");
        let root = repo.root_str();
        let key =
            AdaptiveSecondPassKey::new(QueryClassification::SafeEdit, RepoShapeBucket::LargeDense);
        let stats = AdaptiveSecondPassStats {
            observations: 1,
            missing_slots: 1,
            healed_slots: 0,
            unresolved_slots: 1,
            gain_per_ms_ema: 0.0,
        };

        let persisted = persist_adaptive_second_pass_stats(root.as_str(), &key, &stats);
        assert!(!persisted);
    }

    #[test]
    fn second_pass_subqueries_are_slot_specific() {
        let coverage_audit = CoverageAudit {
            query_classification: QueryClassification::SafeEdit,
            required_slots: vec![
                CoverageSlot::ExactAnchor,
                CoverageSlot::StructuralContext,
                CoverageSlot::VerificationTargets,
            ],
            slot_results: vec![
                CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "present".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::StructuralContext,
                    status: CoverageStatus::MissingRetrievable,
                    detail: "missing".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::VerificationTargets,
                    status: CoverageStatus::MissingRetrievable,
                    detail: "missing".to_string(),
                },
            ],
            sufficient: false,
            summary: "needs second pass".to_string(),
        };

        let subqueries = second_pass_subqueries(
            "RepositoryScanner",
            "what should I verify?",
            QueryClassification::SafeEdit,
            &coverage_audit,
        );
        assert!(subqueries.iter().any(|query| {
            query.slot == CoverageSlot::StructuralContext
                && query
                    .text
                    .contains("import dependency caller callee module relationship")
        }));
        assert!(subqueries.iter().any(|query| {
            query.slot == CoverageSlot::VerificationTargets
                && query
                    .text
                    .contains("test verify invariant regression coverage check")
        }));

        let decision_coverage = CoverageAudit {
            query_classification: QueryClassification::DecisionWhy,
            required_slots: vec![CoverageSlot::ExactAnchor, CoverageSlot::DecisionEvidence],
            slot_results: vec![
                CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "present".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::DecisionEvidence,
                    status: CoverageStatus::MissingRetrievable,
                    detail: "missing".to_string(),
                },
            ],
            sufficient: false,
            summary: "needs decision evidence".to_string(),
        };
        let decision_subqueries = second_pass_subqueries(
            "RepositoryScanner",
            "why is this design used?",
            QueryClassification::DecisionWhy,
            &decision_coverage,
        );
        assert!(decision_subqueries.iter().any(|query| {
            query.slot == CoverageSlot::DecisionEvidence
                && query
                    .text
                    .contains("adr decision rationale tradeoff history architecture record")
        }));
    }

    #[test]
    fn retrieval_frontier_controller_stops_when_forecast_drops_below_floor() {
        let floor =
            class_specific_frontier_floor(LatencyClass::Interactive, QueryClassification::SafeEdit);
        let halted = RetrievalFrontierController::new(floor, Some(floor * 0.5));
        assert!(!halted.should_continue());

        let mut controller = RetrievalFrontierController::new(floor, None);
        assert!(controller.should_continue());
        controller.observe(floor * 0.25);
        assert!(!controller.should_continue());
    }

    #[test]
    fn marginal_gain_threshold_flags_zero_slot_gain_as_too_low() {
        let before = CoverageAudit {
            query_classification: QueryClassification::DecisionWhy,
            required_slots: vec![CoverageSlot::ExactAnchor, CoverageSlot::DecisionEvidence],
            slot_results: vec![
                CoverageSlotAudit {
                    slot: CoverageSlot::ExactAnchor,
                    status: CoverageStatus::Present,
                    detail: "anchor".to_string(),
                },
                CoverageSlotAudit {
                    slot: CoverageSlot::DecisionEvidence,
                    status: CoverageStatus::MissingRetrievable,
                    detail: "missing".to_string(),
                },
            ],
            sufficient: false,
            summary: "before".to_string(),
        };
        let after = before.clone();

        let marginal_gain = marginal_slot_gain_per_ms(&before, &after, 20.0);
        let threshold =
            base_marginal_gain_threshold(LatencyClass::Deep, QueryClassification::DecisionWhy);
        assert!(marginal_gain < threshold);
    }

    fn request_for_scope(scope: &str, token_budget: u32) -> ContextRequest {
        ContextRequest {
            goal: "safe edit".to_string(),
            task_type: TaskType::SafeEdit,
            consumer_type: ConsumerType::Cli,
            model_profile: ModelProfile {
                id: "frontier".to_string(),
                class: ModelClass::FrontierAgent,
                max_context_tokens: 128_000,
                preferred_scaffolding_level: ScaffoldingLevel::Minimal,
                notes: Vec::new(),
            },
            question: "what should I know before editing this?".to_string(),
            scope_hint: Some(scope.to_string()),
            token_budget,
            latency_budget: Some(800),
            depth: RequestDepth::Standard,
            freshness_requirement: Some(FreshnessRequirement::FreshPreferred),
            include_evidence: true,
        }
    }

    fn ready_assessment() -> ReadinessAssessment {
        ReadinessAssessment {
            snapshot_status: SnapshotStatus::Available,
            freshness_status: FreshnessStatus::Current,
            serving_health: ServingHealth::Normal,
        }
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => error.duration().as_nanos(),
        }
    }
}
