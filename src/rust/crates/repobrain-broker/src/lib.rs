use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use repobrain_compiler::{CompilationInput, CompilationPolicy, ContextCompiler, DefaultCompiler};
use repobrain_domain::{
    BriefingItem, BriefingPack, ContextRequest, CoverageAudit, CoverageSlot, CoverageSlotAudit,
    CoverageStatus, EvidenceReceipt, Freshness, FreshnessImpact, ImpactSummary, LatencyClass,
    OverlayClaimScope, OverlayKind, QueryClassification, ReadinessState, TaskType,
    VerificationPlan,
};
use repobrain_graph::{FlowCapsule, GraphStore, SnapshotGraphStore};
use repobrain_ingest::{
    IndexedFile, IndexedSymbol, RepositoryInventorySnapshot, SymbolKind, exact_path_lookup,
    exact_symbol_lookup,
};
use repobrain_serving::{
    HotPathPolicy, ReadinessAssessment, ServingMetadataInput, SnapshotKey, build_serving_metadata,
};
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SnapshotContextBroker {
    hot_path_policy: HotPathPolicy,
}

impl SnapshotContextBroker {
    #[must_use]
    pub fn new(hot_path_policy: HotPathPolicy) -> Self {
        Self { hot_path_policy }
    }

    /// Compiles a deterministic snapshot-backed briefing pack for a scoped request.
    ///
    /// # Errors
    ///
    /// Returns an error when the request does not include a scope hint or the
    /// scope cannot be anchored to an exact path or exact symbol in the
    /// provided snapshot.
    pub fn get_brief(&self, input: SnapshotBrokerInput<'_>) -> Result<BriefingPack, BrokerError> {
        let scope = input
            .request
            .scope_hint
            .clone()
            .ok_or(BrokerError::MissingScopeHint)?;
        let exact_path_hit = exact_path_lookup(input.snapshot, &scope);
        let exact_symbol_hits = exact_symbol_lookup(input.snapshot, &scope);

        if exact_path_hit.is_none() && exact_symbol_hits.is_empty() {
            return Err(BrokerError::UnknownScope(scope));
        }

        let routing = self.hot_path_policy.route(&input.request);
        let graph = SnapshotGraphStore::new(input.snapshot);
        let report = graph.blast_radius(input.request.scope_hint.as_deref().unwrap_or_default());
        let verification_plan = report.verification_plan.clone();
        let evidence_index = collect_evidence_index(&report.evidence, &report.flow_capsules);
        let query_classification = classify_request(&input.request);
        let metadata = serving_metadata_for_broker(&input, &verification_plan);
        let packing_budget = packing_budget(routing.latency_class, input.request.token_budget);
        let suggested_files =
            collect_suggested_files(&report.impacted_nodes, packing_budget.suggested_files);
        let anchor_paths = collect_anchor_paths(exact_path_hit, exact_symbol_hits);
        let freshness = freshness_for(metadata.readiness_state);
        let relevant_decisions = decision_evidence_for_query(
            query_classification,
            input.snapshot,
            input.request.scope_hint.as_deref().unwrap_or_default(),
            &anchor_paths,
            &suggested_files,
        );
        let impact_summary = build_impact_summary(
            input.request.scope_hint.as_deref().unwrap_or_default(),
            &suggested_files,
        );
        let coverage_audit = build_coverage_audit(&CoverageAuditContext {
            query_classification,
            exact_path_hit,
            exact_symbol_hits,
            relationship_count: report.relationships.len(),
            flow_capsules: &report.flow_capsules,
            verification_plan: &verification_plan,
            impact_summary: impact_summary.as_ref(),
            relevant_decisions: &relevant_decisions,
            suggested_files: &suggested_files,
            anchor_path_count: anchor_paths.len(),
        });
        let must_know = collect_must_know(&MustKnowContext {
            scope: input.request.scope_hint.as_deref().unwrap_or_default(),
            exact_path_hit,
            exact_symbol_hits,
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

fn decision_evidence_for_query(
    query_classification: QueryClassification,
    snapshot: &RepositoryInventorySnapshot,
    scope: &str,
    anchor_paths: &BTreeSet<String>,
    suggested_files: &[String],
) -> Vec<String> {
    if query_classification != QueryClassification::DecisionWhy {
        return Vec::new();
    }

    collect_relevant_decisions(snapshot, scope, anchor_paths, suggested_files, 4)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BrokerError {
    #[error("get_brief requires a scope hint for the current deterministic slice")]
    MissingScopeHint,
    #[error("scope `{0}` was not found in the current snapshot")]
    UnknownScope(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PackingBudget {
    brief_items: usize,
    suggested_files: usize,
    fragile_zones: usize,
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

fn collect_relevant_decisions(
    snapshot: &RepositoryInventorySnapshot,
    scope: &str,
    anchor_paths: &BTreeSet<String>,
    suggested_files: &[String],
    limit: usize,
) -> Vec<String> {
    const MAX_DECISION_DOC_BYTES: u64 = 256 * 1024;
    const MAX_DECISION_SNIPPET_CHARS: usize = 160;

    let scope_terms = text_terms(scope);
    let anchor_terms = anchor_paths
        .iter()
        .flat_map(|path| path_terms(path))
        .collect::<BTreeSet<_>>();
    let suggested_terms = suggested_files
        .iter()
        .flat_map(|path| path_terms(path))
        .collect::<BTreeSet<_>>();
    let mut scored = Vec::<(usize, String, String)>::new();

    for file in &snapshot.files {
        if file.size_bytes > MAX_DECISION_DOC_BYTES {
            continue;
        }
        if !looks_like_decision_doc(
            &file.relative_path,
            file.language.as_deref(),
            file.extension.as_deref(),
        ) {
            continue;
        }

        let path = Path::new(&snapshot.root).join(&file.relative_path);
        let Ok(content_profile) = scan_decision_document(
            &path,
            &scope_terms,
            &anchor_terms,
            &suggested_terms,
            MAX_DECISION_SNIPPET_CHARS,
        ) else {
            continue;
        };
        if !content_profile.has_markers {
            continue;
        }

        let ranking_score = decision_doc_score(&file.relative_path, &content_profile);
        let title = content_profile
            .title
            .unwrap_or_else(|| file.relative_path.clone());
        let snippet = content_profile
            .snippet
            .unwrap_or_else(|| "decision context is documented in this file".to_string());
        let rendered = format!("{title} [{}]: {snippet}", file.relative_path);
        scored.push((
            usize::MAX - ranking_score,
            file.relative_path.clone(),
            rendered,
        ));
    }

    scored.sort_unstable();
    scored
        .into_iter()
        .map(|(_, _, rendered)| rendered)
        .take(limit)
        .collect()
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

    score += content_profile.scope_overlap * 25;
    score += content_profile.anchor_overlap * 10;
    score += content_profile.suggested_overlap * 6;

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

struct MustKnowContext<'a> {
    scope: &'a str,
    exact_path_hit: Option<&'a IndexedFile>,
    exact_symbol_hits: &'a [IndexedSymbol],
    suggested_files: &'a [String],
    flow_capsules: &'a [FlowCapsule],
    evidence_index: &'a [EvidenceReceipt],
    verification_plan: &'a VerificationPlan,
    freshness: Freshness,
    coverage_audit: &'a CoverageAudit,
}

struct CoverageAuditContext<'a> {
    query_classification: QueryClassification,
    exact_path_hit: Option<&'a IndexedFile>,
    exact_symbol_hits: &'a [IndexedSymbol],
    relationship_count: usize,
    flow_capsules: &'a [FlowCapsule],
    verification_plan: &'a VerificationPlan,
    impact_summary: Option<&'a ImpactSummary>,
    relevant_decisions: &'a [String],
    suggested_files: &'a [String],
    anchor_path_count: usize,
}

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
        .all(|result| result.status == CoverageStatus::Satisfied);
    let incomplete_slots = slot_results
        .iter()
        .filter(|result| result.status != CoverageStatus::Satisfied)
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
            "Coverage audit for `{}` is incomplete; missing or partial slots: {}.",
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
    let status = if path_hits + symbol_hits > 0 {
        CoverageStatus::Satisfied
    } else {
        CoverageStatus::Missing
    };

    CoverageSlotAudit {
        slot: CoverageSlot::ExactAnchor,
        status,
        detail: format!(
            "exact anchoring found {path_hits} path hit(s) and {symbol_hits} symbol hit(s)"
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
            CoverageStatus::Satisfied,
            format!(
                "structural context covers {non_anchor_files} non-anchor file(s) and {} relationship(s)",
                context.relationship_count
            ),
        )
    } else if !context.suggested_files.is_empty() {
        (
            CoverageStatus::Partial,
            "exact anchor is known, but no non-anchor structural neighbors were found".to_string(),
        )
    } else {
        (
            CoverageStatus::Missing,
            "no structural context was assembled for this scope".to_string(),
        )
    };

    CoverageSlotAudit {
        slot: CoverageSlot::StructuralContext,
        status,
        detail,
    }
}

fn audit_flow_summary(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    CoverageSlotAudit {
        slot: CoverageSlot::FlowSummary,
        status: if context.flow_capsules.is_empty() {
            CoverageStatus::Missing
        } else {
            CoverageStatus::Satisfied
        },
        detail: format!(
            "{} flow capsule(s) are available for this scope",
            context.flow_capsules.len()
        ),
    }
}

fn audit_verification_targets(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let required_count = context.verification_plan.required_checks.len();
    let recommended_count = context.verification_plan.recommended_checks.len();
    let status = if required_count > 0 {
        CoverageStatus::Satisfied
    } else if recommended_count > 0 {
        CoverageStatus::Partial
    } else {
        CoverageStatus::Missing
    };

    CoverageSlotAudit {
        slot: CoverageSlot::VerificationTargets,
        status,
        detail: format!(
            "verification planning found {required_count} required and {recommended_count} recommended check(s)"
        ),
    }
}

fn audit_impact_envelope(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    let Some(impact_summary) = context.impact_summary else {
        return CoverageSlotAudit {
            slot: CoverageSlot::ImpactEnvelope,
            status: CoverageStatus::Missing,
            detail: "no impact summary was compiled for this scope".to_string(),
        };
    };

    CoverageSlotAudit {
        slot: CoverageSlot::ImpactEnvelope,
        status: if impact_summary.changed_scope.is_empty() {
            CoverageStatus::Partial
        } else {
            CoverageStatus::Satisfied
        },
        detail: format!(
            "impact envelope covers {} changed scope file(s)",
            impact_summary.changed_scope.len()
        ),
    }
}

fn audit_decision_evidence(context: &CoverageAuditContext<'_>) -> CoverageSlotAudit {
    CoverageSlotAudit {
        slot: CoverageSlot::DecisionEvidence,
        status: if context.relevant_decisions.is_empty() {
            CoverageStatus::Missing
        } else {
            CoverageStatus::Satisfied
        },
        detail: if context.relevant_decisions.is_empty() {
            "no deterministic decision evidence is available in the current slice".to_string()
        } else {
            format!(
                "{} decision evidence item(s) are available for this scope",
                context.relevant_decisions.len()
            )
        },
    }
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
    evidence_index
        .iter()
        .filter(|receipt| receipt.source_type == "verification_target")
        .filter(|receipt| checks.contains(&receipt.source_ref))
        .map(|receipt| receipt.id.clone())
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
        ContextRequest, CoverageSlot, FreshnessRequirement, LatencyClass, ModelClass, ModelProfile,
        OverlayClaimScope, OverlayKind, QueryClassification, ReadinessState, RequestDepth,
        ScaffoldingLevel, TaskType,
    };
    use repobrain_ingest::{RepositoryScanner, RepositoryTarget};
    use repobrain_serving::{FreshnessStatus, ReadinessAssessment, ServingHealth, SnapshotStatus};

    use super::{BrokerError, SnapshotBrokerInput, SnapshotContextBroker, packing_budget};

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

    fn request_for_scope(scope: &str, token_budget: u32) -> ContextRequest {
        ContextRequest {
            goal: "safe edit".to_string(),
            task_type: TaskType::SafeEdit,
            consumer_type: "cli".to_string(),
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
