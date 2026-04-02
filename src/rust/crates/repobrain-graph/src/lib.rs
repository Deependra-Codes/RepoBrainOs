use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use repobrain_domain::{EvidenceReceipt, VerificationPlan};
use repobrain_ingest::{
    IndexedImport, IndexedSymbol, RepositoryInventorySnapshot, VerificationTargetPriority,
    direct_import_lookup, exact_path_lookup, exact_symbol_lookup, reverse_import_lookup,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphEdgeKind {
    Contains,
    Defines,
    Imports,
    References,
    Calls,
    Builds,
    Tests,
    Documents,
    SupportsDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphTraversalRequest {
    pub seed: String,
    pub max_hops: u8,
    pub edge_kinds: Vec<GraphEdgeKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphHit {
    pub node_ref: String,
    pub score: f32,
    pub why: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphRelationship {
    pub edge_kind: GraphEdgeKind,
    pub source_ref: String,
    pub target_ref: String,
    pub summary: String,
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlastRadiusReport {
    pub target: String,
    pub impacted_nodes: Vec<String>,
    pub relationships: Vec<GraphRelationship>,
    pub invariants: Vec<String>,
    pub flow_capsules: Vec<FlowCapsule>,
    pub verification_plan: VerificationPlan,
    pub verification_targets: Vec<String>,
    pub evidence: Vec<EvidenceReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlowCapsule {
    pub name: String,
    pub entrypoints: Vec<String>,
    pub core_modules: Vec<String>,
    pub state_transitions: Vec<String>,
    pub side_effects: Vec<String>,
    pub failure_modes: Vec<String>,
    pub tests_covering_flow: Vec<String>,
    pub evidence: Vec<EvidenceReceipt>,
}

const MAX_ANCHOR_PATH_INVARIANTS: usize = 4;
const MAX_SYMBOL_INVARIANTS: usize = 4;
const MAX_STRUCTURAL_EDGE_INVARIANTS: usize = 6;
const MAX_REQUIRED_CHECK_INVARIANTS: usize = 4;
const MAX_CHECK_RELATIONSHIPS: usize = 24;
const MAX_DOCUMENT_RELATIONSHIPS_PER_DOC: usize = 3;

pub trait GraphStore {
    fn traverse(&self, request: &GraphTraversalRequest) -> Vec<GraphHit>;
    fn blast_radius(&self, target: &str) -> BlastRadiusReport;
}

pub struct SnapshotGraphStore<'a> {
    snapshot: &'a RepositoryInventorySnapshot,
}

impl<'a> SnapshotGraphStore<'a> {
    #[must_use]
    pub fn new(snapshot: &'a RepositoryInventorySnapshot) -> Self {
        Self { snapshot }
    }

    fn seed_paths(&self, target: &str) -> BTreeSet<String> {
        let mut paths = BTreeSet::new();

        if let Some(file) = exact_path_lookup(self.snapshot, target) {
            let _ = paths.insert(file.relative_path.clone());
        }

        for symbol in exact_symbol_lookup(self.snapshot, target) {
            let _ = paths.insert(symbol.relative_path.clone());
        }

        paths
    }

    fn seed_symbols(&self, target: &str) -> Vec<&IndexedSymbol> {
        exact_symbol_lookup(self.snapshot, target).iter().collect()
    }

    fn impacted_paths(&self, seed_paths: &BTreeSet<String>) -> BTreeSet<String> {
        let mut impacted = seed_paths.clone();

        for seed_path in seed_paths {
            for import in direct_import_lookup(self.snapshot, seed_path) {
                if let Some(resolved_path) = &import.resolved_path {
                    let _ = impacted.insert(resolved_path.clone());
                }
            }

            for import in reverse_import_lookup(self.snapshot, seed_path) {
                let _ = impacted.insert(import.importer_path.clone());
            }
        }

        impacted
    }

    fn defined_symbols_for_paths<'snapshot>(
        &'snapshot self,
        paths: &BTreeSet<String>,
    ) -> Vec<&'snapshot IndexedSymbol> {
        paths
            .iter()
            .flat_map(|path| self.snapshot.symbols_for_path(path))
            .collect()
    }

    fn reference_imports_touching_seed_paths<'snapshot>(
        &'snapshot self,
        seed_paths: &BTreeSet<String>,
        impacted_paths: &BTreeSet<String>,
    ) -> Vec<&'snapshot IndexedImport> {
        let mut references = Vec::new();
        let mut seen = BTreeSet::new();

        for seed_path in seed_paths {
            for import in direct_import_lookup(self.snapshot, seed_path) {
                let Some(resolved_path) = &import.resolved_path else {
                    continue;
                };
                if impacted_paths.contains(resolved_path) && seen.insert(import.fact_id.as_str()) {
                    references.push(import);
                }
            }

            for import in reverse_import_lookup(self.snapshot, seed_path) {
                if seen.insert(import.fact_id.as_str()) {
                    references.push(import);
                }
            }
        }

        references
    }

    fn symbol_anchor_nodes(seed_symbols: &[&IndexedSymbol]) -> Vec<String> {
        seed_symbols
            .iter()
            .map(|symbol| symbol_node_ref(symbol))
            .collect()
    }

    fn collect_relationships(
        &self,
        target: &str,
        impacted_paths: &BTreeSet<String>,
        defined_symbols: &[&IndexedSymbol],
        reference_imports: &[&IndexedImport],
        verification_plan: &VerificationPlan,
        decision_documents: &[String],
    ) -> Vec<GraphRelationship> {
        let mut relationships = Self::define_relationships(defined_symbols);
        relationships.extend(Self::reference_and_import_relationships(reference_imports));
        relationships.extend(self.call_relationships(reference_imports));
        relationships.extend(Self::check_relationships(impacted_paths, verification_plan));
        relationships.extend(Self::document_relationships(
            target,
            impacted_paths,
            decision_documents,
        ));

        normalize_relationships(relationships)
    }

    fn define_relationships(defined_symbols: &[&IndexedSymbol]) -> Vec<GraphRelationship> {
        defined_symbols
            .iter()
            .map(|symbol| GraphRelationship {
                edge_kind: GraphEdgeKind::Defines,
                source_ref: file_node_ref(&symbol.relative_path),
                target_ref: symbol_node_ref(symbol),
                summary: format!(
                    "{} defines {}",
                    symbol.relative_path,
                    symbol_display(symbol)
                ),
                evidence_ids: vec![symbol_definition_receipt_id(symbol)],
            })
            .collect()
    }

    fn reference_and_import_relationships(
        reference_imports: &[&IndexedImport],
    ) -> Vec<GraphRelationship> {
        let mut relationships = Vec::new();

        for import in reference_imports {
            let Some(resolved_path) = &import.resolved_path else {
                continue;
            };
            let evidence_ids = vec![reference_edge_receipt_id(import)];
            relationships.push(GraphRelationship {
                edge_kind: GraphEdgeKind::References,
                source_ref: file_node_ref(&import.importer_path),
                target_ref: file_node_ref(resolved_path),
                summary: format!(
                    "{} references {} via `{}`",
                    import.importer_path, resolved_path, import.import_spec
                ),
                evidence_ids: evidence_ids.clone(),
            });
            relationships.push(GraphRelationship {
                edge_kind: GraphEdgeKind::Imports,
                source_ref: file_node_ref(&import.importer_path),
                target_ref: file_node_ref(resolved_path),
                summary: format!(
                    "{} imports {} via `{}`",
                    import.importer_path, resolved_path, import.import_spec
                ),
                evidence_ids,
            });
        }

        relationships
    }

    fn call_relationships(&self, reference_imports: &[&IndexedImport]) -> Vec<GraphRelationship> {
        let mut relationships = Vec::new();
        for import in reference_imports {
            let Some(target_symbol) = self.call_target_for_import(import) else {
                continue;
            };
            let Some(source_symbol) = self.call_source_for_import(import) else {
                continue;
            };
            relationships.push(GraphRelationship {
                edge_kind: GraphEdgeKind::Calls,
                source_ref: symbol_node_ref(source_symbol),
                target_ref: symbol_node_ref(target_symbol),
                summary: format!(
                    "{} calls {} via `{}`",
                    symbol_display(source_symbol),
                    symbol_display(target_symbol),
                    import.import_spec
                ),
                evidence_ids: vec![reference_edge_receipt_id(import)],
            });
        }

        relationships
    }

    fn check_relationships(
        impacted_paths: &BTreeSet<String>,
        verification_plan: &VerificationPlan,
    ) -> Vec<GraphRelationship> {
        let mut relationships = Vec::new();
        let mut check_edges = 0_usize;
        for check in verification_plan
            .required_checks
            .iter()
            .chain(&verification_plan.recommended_checks)
        {
            if check_edges >= MAX_CHECK_RELATIONSHIPS {
                break;
            }
            let edge_kind = if looks_like_test_check(check) {
                GraphEdgeKind::Tests
            } else {
                GraphEdgeKind::Builds
            };
            let check_node = verification_node_ref(check);
            let evidence_ids = vec![verification_target_receipt_id(check)];
            for path in impacted_paths {
                relationships.push(GraphRelationship {
                    edge_kind,
                    source_ref: file_node_ref(path),
                    target_ref: check_node.clone(),
                    summary: format!(
                        "{} {} via `{}`",
                        path,
                        relationship_kind_label(edge_kind),
                        check
                    ),
                    evidence_ids: evidence_ids.clone(),
                });
                check_edges += 1;
                if check_edges >= MAX_CHECK_RELATIONSHIPS {
                    break;
                }
            }
        }

        relationships
    }

    fn document_relationships(
        target: &str,
        impacted_paths: &BTreeSet<String>,
        decision_documents: &[String],
    ) -> Vec<GraphRelationship> {
        let mut relationships = Vec::new();
        for doc_path in decision_documents {
            let doc_ref = file_node_ref(doc_path);
            let evidence_ids = vec![decision_document_receipt_id(doc_path)];
            for impacted in impacted_paths
                .iter()
                .take(MAX_DOCUMENT_RELATIONSHIPS_PER_DOC)
            {
                relationships.push(GraphRelationship {
                    edge_kind: GraphEdgeKind::Documents,
                    source_ref: doc_ref.clone(),
                    target_ref: file_node_ref(impacted),
                    summary: format!("{doc_path} documents decisions relevant to {impacted}"),
                    evidence_ids: evidence_ids.clone(),
                });
            }

            relationships.push(GraphRelationship {
                edge_kind: GraphEdgeKind::SupportsDecision,
                source_ref: doc_ref,
                target_ref: decision_scope_ref(target),
                summary: format!("{doc_path} supports decision context for `{target}`"),
                evidence_ids,
            });
        }

        relationships
    }

    fn call_target_for_import(&self, import: &IndexedImport) -> Option<&IndexedSymbol> {
        let resolved_path = import.resolved_path.as_deref()?;
        let symbol_name = import_leaf_symbol(&import.import_spec)?;
        self.snapshot
            .symbols_for_path(resolved_path)
            .filter(|symbol| matches!(symbol.kind, repobrain_ingest::SymbolKind::Function))
            .find(|symbol| symbol.name.eq_ignore_ascii_case(symbol_name))
    }

    fn call_source_for_import(&self, import: &IndexedImport) -> Option<&IndexedSymbol> {
        self.snapshot
            .symbols_for_path(&import.importer_path)
            .filter(|symbol| matches!(symbol.kind, repobrain_ingest::SymbolKind::Function))
            .filter(|symbol| symbol.line_number <= import.line_number)
            .max_by_key(|symbol| symbol.line_number)
            .or_else(|| {
                self.snapshot
                    .symbols_for_path(&import.importer_path)
                    .find(|symbol| matches!(symbol.kind, repobrain_ingest::SymbolKind::Function))
            })
    }

    fn collect_decision_documents(
        &self,
        target: &str,
        impacted_paths: &BTreeSet<String>,
        limit: usize,
    ) -> Vec<String> {
        const MAX_DECISION_DOC_BYTES: u64 = 256 * 1024;

        let target_lower = target.to_ascii_lowercase();
        let impacted_terms = impacted_paths
            .iter()
            .flat_map(|path| decision_terms_for_path(path))
            .collect::<BTreeSet<_>>();
        let mut scored = Vec::<(usize, String)>::new();

        for file in &self.snapshot.files {
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

            let path_lower = file.relative_path.to_ascii_lowercase();
            let mut score = 0_usize;
            if path_lower.contains("adr") {
                score += 40;
            }
            if path_lower.contains("decision") {
                score += 30;
            }
            if path_lower.contains(&target_lower) {
                score += 24;
            }
            for term in &impacted_terms {
                if path_lower.contains(term.as_str()) {
                    score += 6;
                }
            }

            let path = Path::new(&self.snapshot.root).join(&file.relative_path);
            let Ok(content_signals) =
                scan_decision_document_content(&path, &target_lower, &impacted_terms)
            else {
                continue;
            };
            if !content_signals.has_markers {
                continue;
            }

            if content_signals.target_hits > 0 {
                score += 18;
            }
            score += content_signals.impacted_overlap * 4;

            scored.push((usize::MAX - score, file.relative_path.clone()));
        }

        scored.sort_unstable();
        scored
            .into_iter()
            .map(|(_, path)| path)
            .take(limit)
            .collect()
    }

    fn collect_evidence(
        &self,
        target: &str,
        defined_symbols: &[&IndexedSymbol],
        reference_imports: &[&IndexedImport],
        verification_plan: &VerificationPlan,
        decision_documents: &[String],
    ) -> Vec<EvidenceReceipt> {
        let mut evidence = Vec::new();

        if let Some(file) = exact_path_lookup(self.snapshot, target) {
            evidence.push(make_receipt(
                "file_anchor",
                &file.relative_path,
                "path",
                &self.snapshot.captured_at,
            ));
        }

        for symbol in defined_symbols {
            evidence.push(symbol_definition_receipt(
                symbol,
                &self.snapshot.captured_at,
            ));
        }

        for import in reference_imports {
            evidence.push(reference_edge_receipt(import, &self.snapshot.captured_at));
        }

        for check in verification_plan
            .required_checks
            .iter()
            .chain(&verification_plan.recommended_checks)
        {
            evidence.push(make_receipt(
                "verification_target",
                check,
                "planned",
                &self.snapshot.captured_at,
            ));
        }

        for document in decision_documents {
            evidence.push(decision_document_receipt(
                document,
                &self.snapshot.captured_at,
            ));
        }

        normalize_evidence(evidence)
    }

    fn build_flow_capsules(
        target: &str,
        seed_symbols: &[&IndexedSymbol],
        impacted_paths: &BTreeSet<String>,
        relationships: &[GraphRelationship],
        verification_plan: &VerificationPlan,
        evidence: &[EvidenceReceipt],
    ) -> Vec<FlowCapsule> {
        if impacted_paths.is_empty() {
            return Vec::new();
        }

        vec![FlowCapsule {
            name: format!("structural flow around `{target}`"),
            entrypoints: flow_entrypoints(target, seed_symbols, impacted_paths),
            core_modules: impacted_paths.iter().cloned().collect(),
            state_transitions: relationship_transitions(relationships),
            side_effects: Vec::new(),
            failure_modes: verification_plan.stop_conditions.clone(),
            tests_covering_flow: verification_plan
                .required_checks
                .iter()
                .filter(|check| looks_like_test_check(check))
                .cloned()
                .collect(),
            evidence: evidence
                .iter()
                .filter(|receipt| {
                    matches!(
                        receipt.source_type.as_str(),
                        "file_anchor"
                            | "symbol_definition"
                            | "reference_edge"
                            | "decision_document"
                    )
                })
                .cloned()
                .collect(),
        }]
    }

    fn plan_verification(&self, impacted_paths: &BTreeSet<String>) -> VerificationPlan {
        let mut required_checks = BTreeSet::new();
        let mut recommended_checks = BTreeSet::new();
        let mut covered_paths = BTreeSet::new();

        for target in self.snapshot.verification_targets() {
            if !matches_any_scope(impacted_paths, &target.scope_root) {
                continue;
            }

            let rendered = render_check(target.working_directory.as_str(), &target.command);
            match target.priority {
                VerificationTargetPriority::Required => {
                    let _ = required_checks.insert(rendered);
                }
                VerificationTargetPriority::Recommended => {
                    let _ = recommended_checks.insert(rendered);
                }
            }

            if target.scope_root.is_empty() {
                covered_paths.extend(impacted_paths.iter().cloned());
            } else {
                for path in impacted_paths {
                    if path_matches_scope(path, &target.scope_root) {
                        let _ = covered_paths.insert(path.clone());
                    }
                }
            }
        }

        let coverage_gaps = impacted_paths
            .iter()
            .filter(|path| !covered_paths.contains(*path))
            .map(|path| format!("no deterministic scoped verification target found for `{path}`"))
            .collect::<Vec<_>>();
        let stop_conditions = if required_checks.is_empty() {
            vec![
                "no deterministic required checks were discovered for this blast radius; narrow scope or add manual verification".to_string(),
            ]
        } else {
            Vec::new()
        };

        VerificationPlan {
            required_checks: required_checks.into_iter().collect(),
            recommended_checks: recommended_checks.into_iter().collect(),
            invariants: Vec::new(),
            coverage_gaps,
            stop_conditions,
        }
    }

    fn derive_invariants(
        target: &str,
        seed_paths: &BTreeSet<String>,
        seed_symbols: &[&IndexedSymbol],
        reference_imports: &[&IndexedImport],
        verification_plan: &VerificationPlan,
    ) -> Vec<String> {
        let mut invariants = BTreeSet::new();

        if !seed_paths.is_empty() {
            let _ = invariants.insert(format!(
                "scope around `{target}` remains bounded to snapshot-confirmed direct imports and reverse importers"
            ));
        }

        for symbol in seed_symbols.iter().take(MAX_SYMBOL_INVARIANTS) {
            let _ = invariants.insert(format!(
                "symbol `{}` remains defined at `{}`:{}",
                symbol.name, symbol.relative_path, symbol.line_number
            ));
        }

        for path in seed_paths.iter().take(MAX_ANCHOR_PATH_INVARIANTS) {
            let _ = invariants.insert(format!(
                "anchor path `{path}` remains part of the scoped change envelope"
            ));
        }

        let edge_invariants = reference_imports
            .iter()
            .filter_map(|import| {
                let resolved_path = import.resolved_path.as_deref()?;
                Some(format!(
                    "direct structural edge `{}` -> `{}` remains intact",
                    import.importer_path, resolved_path
                ))
            })
            .collect::<BTreeSet<_>>();
        for invariant in edge_invariants
            .into_iter()
            .take(MAX_STRUCTURAL_EDGE_INVARIANTS)
        {
            let _ = invariants.insert(invariant);
        }

        for check in verification_plan
            .required_checks
            .iter()
            .take(MAX_REQUIRED_CHECK_INVARIANTS)
        {
            let _ = invariants.insert(format!("required verification remains passing: `{check}`"));
        }

        invariants.into_iter().collect()
    }
}

impl GraphStore for SnapshotGraphStore<'_> {
    fn traverse(&self, request: &GraphTraversalRequest) -> Vec<GraphHit> {
        let mut hits = Vec::new();
        let seed_paths = self.seed_paths(&request.seed);

        if request.edge_kinds.contains(&GraphEdgeKind::Contains) {
            for symbol in exact_symbol_lookup(self.snapshot, &request.seed) {
                hits.push(GraphHit {
                    node_ref: format!("file:{}", symbol.relative_path),
                    score: 1.0,
                    why: format!(
                        "exact symbol ownership for `{}` at line {}",
                        symbol.name, symbol.line_number
                    ),
                });
            }
        }

        if request.edge_kinds.contains(&GraphEdgeKind::Defines) {
            if let Some(file) = exact_path_lookup(self.snapshot, &request.seed) {
                for symbol in self.snapshot.symbols_for_path(&file.relative_path) {
                    hits.push(GraphHit {
                        node_ref: symbol_node_ref(symbol),
                        score: 1.0,
                        why: format!("`{}` defines `{}`", file.relative_path, symbol.name),
                    });
                }
            }

            for symbol in exact_symbol_lookup(self.snapshot, &request.seed) {
                hits.push(GraphHit {
                    node_ref: file_node_ref(&symbol.relative_path),
                    score: 1.0,
                    why: format!(
                        "stable symbol fact `{}` is defined in `{}`",
                        symbol.fact_id, symbol.relative_path
                    ),
                });
            }
        }

        if request.edge_kinds.contains(&GraphEdgeKind::Imports) {
            for seed_path in &seed_paths {
                for import in direct_import_lookup(self.snapshot, seed_path) {
                    if let Some(resolved_path) = &import.resolved_path {
                        hits.push(GraphHit {
                            node_ref: format!("file:{resolved_path}"),
                            score: 0.8,
                            why: format!(
                                "{} import `{}` on line {}",
                                import.language, import.import_spec, import.line_number
                            ),
                        });
                    }
                }

                for import in reverse_import_lookup(self.snapshot, seed_path) {
                    hits.push(GraphHit {
                        node_ref: format!("file:{}", import.importer_path),
                        score: 0.8,
                        why: format!(
                            "{} importer `{}` on line {}",
                            import.language, import.import_spec, import.line_number
                        ),
                    });
                }
            }
        }

        if request.edge_kinds.contains(&GraphEdgeKind::References) {
            for seed_path in &seed_paths {
                for import in direct_import_lookup(self.snapshot, seed_path) {
                    if let Some(resolved_path) = &import.resolved_path {
                        hits.push(GraphHit {
                            node_ref: file_node_ref(resolved_path),
                            score: 0.8,
                            why: format!(
                                "{} syntax reference `{}` on line {}",
                                import.language, import.import_spec, import.line_number
                            ),
                        });
                    }
                }
            }
        }

        dedupe_hits(hits)
    }

    fn blast_radius(&self, target: &str) -> BlastRadiusReport {
        let seed_symbols = self.seed_symbols(target);
        let seed_paths = self.seed_paths(target);
        let impacted_paths = self.impacted_paths(&seed_paths);
        let defined_symbols = if seed_symbols.is_empty() {
            self.defined_symbols_for_paths(&seed_paths)
        } else {
            seed_symbols.clone()
        };
        let reference_imports =
            self.reference_imports_touching_seed_paths(&seed_paths, &impacted_paths);
        let mut verification_plan = self.plan_verification(&impacted_paths);
        let decision_documents = self.collect_decision_documents(target, &impacted_paths, 4);
        let relationships = self.collect_relationships(
            target,
            &impacted_paths,
            &defined_symbols,
            &reference_imports,
            &verification_plan,
            &decision_documents,
        );
        let symbol_nodes = Self::symbol_anchor_nodes(&seed_symbols);
        let mut impacted_nodes = symbol_nodes;
        impacted_nodes.extend(impacted_paths.iter().map(|path| file_node_ref(path)));
        let invariants = Self::derive_invariants(
            target,
            &seed_paths,
            &seed_symbols,
            &reference_imports,
            &verification_plan,
        );
        verification_plan.invariants.clone_from(&invariants);
        let evidence = self.collect_evidence(
            target,
            &defined_symbols,
            &reference_imports,
            &verification_plan,
            &decision_documents,
        );
        let flow_capsules = Self::build_flow_capsules(
            target,
            &seed_symbols,
            &impacted_paths,
            &relationships,
            &verification_plan,
            &evidence,
        );
        let verification_targets = verification_plan
            .required_checks
            .iter()
            .chain(&verification_plan.recommended_checks)
            .cloned()
            .collect::<Vec<_>>();

        BlastRadiusReport {
            target: target.to_string(),
            impacted_nodes,
            relationships,
            invariants,
            flow_capsules,
            verification_plan,
            verification_targets,
            evidence,
        }
    }
}

#[must_use]
pub fn seed_blast_radius_report(target: impl Into<String>) -> BlastRadiusReport {
    BlastRadiusReport {
        target: target.into(),
        impacted_nodes: Vec::new(),
        relationships: Vec::new(),
        invariants: Vec::new(),
        flow_capsules: Vec::new(),
        verification_plan: VerificationPlan {
            required_checks: Vec::new(),
            recommended_checks: Vec::new(),
            invariants: Vec::new(),
            coverage_gaps: Vec::new(),
            stop_conditions: Vec::new(),
        },
        verification_targets: Vec::new(),
        evidence: Vec::new(),
    }
}

fn dedupe_hits(mut hits: Vec<GraphHit>) -> Vec<GraphHit> {
    hits.sort_unstable_by(|left, right| left.node_ref.cmp(&right.node_ref));
    hits.dedup_by(|left, right| left.node_ref == right.node_ref);
    hits
}

fn normalize_relationships(mut relationships: Vec<GraphRelationship>) -> Vec<GraphRelationship> {
    for relationship in &mut relationships {
        relationship.evidence_ids.sort_unstable();
        relationship.evidence_ids.dedup();
    }

    relationships.sort_unstable_by(|left, right| {
        left.edge_kind
            .cmp(&right.edge_kind)
            .then_with(|| left.source_ref.cmp(&right.source_ref))
            .then_with(|| left.target_ref.cmp(&right.target_ref))
            .then_with(|| left.summary.cmp(&right.summary))
    });
    relationships.dedup_by(|left, right| {
        left.edge_kind == right.edge_kind
            && left.source_ref == right.source_ref
            && left.target_ref == right.target_ref
            && left.summary == right.summary
    });
    relationships
}

fn matches_any_scope(paths: &BTreeSet<String>, scope_root: &str) -> bool {
    if scope_root.is_empty() {
        return !paths.is_empty();
    }

    paths
        .iter()
        .any(|path| path_matches_scope(path, scope_root))
}

fn path_matches_scope(path: &str, scope_root: &str) -> bool {
    path == scope_root || path.starts_with(&format!("{scope_root}/"))
}

fn render_check(working_directory: &str, command: &[String]) -> String {
    let rendered_command = command.join(" ");
    if working_directory == "." {
        rendered_command
    } else {
        format!("{rendered_command} (in {working_directory})")
    }
}

fn flow_entrypoints(
    _target: &str,
    seed_symbols: &[&IndexedSymbol],
    seed_paths: &BTreeSet<String>,
) -> Vec<String> {
    let symbol_entrypoints = seed_symbols
        .iter()
        .map(|symbol| symbol_display(symbol))
        .collect::<Vec<_>>();
    if !symbol_entrypoints.is_empty() {
        return symbol_entrypoints;
    }

    seed_paths
        .iter()
        .map(|path| file_node_ref(path))
        .collect::<Vec<_>>()
}

fn relationship_transitions(relationships: &[GraphRelationship]) -> Vec<String> {
    relationships
        .iter()
        .map(|relationship| relationship.summary.clone())
        .collect()
}

fn looks_like_test_check(check: &str) -> bool {
    check.contains(" test") || check.contains("unittest") || check.contains("pytest")
}

fn normalize_evidence(mut evidence: Vec<EvidenceReceipt>) -> Vec<EvidenceReceipt> {
    evidence.sort_unstable_by(|left, right| left.id.cmp(&right.id));
    evidence.dedup_by(|left, right| left.id == right.id);
    evidence
}

fn file_node_ref(path: &str) -> String {
    format!("file:{path}")
}

fn symbol_node_ref(symbol: &IndexedSymbol) -> String {
    format!("symbol:{}", symbol.fact_id)
}

fn symbol_display(symbol: &IndexedSymbol) -> String {
    format!("symbol:{}@{}", symbol.name, symbol.relative_path)
}

fn symbol_definition_receipt(symbol: &IndexedSymbol, captured_at: &str) -> EvidenceReceipt {
    EvidenceReceipt {
        id: symbol_definition_receipt_id(symbol),
        source_type: "symbol_definition".to_string(),
        source_ref: symbol.fact_id.clone(),
        locator: format!(
            "{}:L{}:symbol:{}",
            symbol.relative_path, symbol.line_number, symbol.name
        ),
        snippet_hash: None,
        captured_at: captured_at.to_string(),
    }
}

fn symbol_definition_receipt_id(symbol: &IndexedSymbol) -> String {
    format!("ev_symbol_definition_{}", sanitize_for_id(&symbol.fact_id))
}

fn reference_edge_receipt(import: &IndexedImport, captured_at: &str) -> EvidenceReceipt {
    EvidenceReceipt {
        id: reference_edge_receipt_id(import),
        source_type: "reference_edge".to_string(),
        source_ref: import.fact_id.clone(),
        locator: format!(
            "{}:L{}:ref:{}",
            import.importer_path, import.line_number, import.import_spec
        ),
        snippet_hash: None,
        captured_at: captured_at.to_string(),
    }
}

fn reference_edge_receipt_id(import: &IndexedImport) -> String {
    format!("ev_reference_edge_{}", sanitize_for_id(&import.fact_id))
}

fn decision_document_receipt(document_path: &str, captured_at: &str) -> EvidenceReceipt {
    EvidenceReceipt {
        id: decision_document_receipt_id(document_path),
        source_type: "decision_document".to_string(),
        source_ref: document_path.to_string(),
        locator: "decision".to_string(),
        snippet_hash: None,
        captured_at: captured_at.to_string(),
    }
}

fn decision_document_receipt_id(document_path: &str) -> String {
    format!("ev_decision_document_{}", sanitize_for_id(document_path))
}

fn verification_target_receipt_id(check: &str) -> String {
    make_receipt(
        "verification_target",
        check,
        "planned",
        "1970-01-01T00:00:00Z",
    )
    .id
}

fn verification_node_ref(check: &str) -> String {
    format!("check:{}", sanitize_for_id(check))
}

fn decision_scope_ref(target: &str) -> String {
    format!("scope:{}", sanitize_for_id(target))
}

fn relationship_kind_label(kind: GraphEdgeKind) -> &'static str {
    match kind {
        GraphEdgeKind::Contains => "contains",
        GraphEdgeKind::Defines => "defines",
        GraphEdgeKind::Imports => "imports",
        GraphEdgeKind::References => "references",
        GraphEdgeKind::Calls => "calls",
        GraphEdgeKind::Builds => "builds",
        GraphEdgeKind::Tests => "tests",
        GraphEdgeKind::Documents => "documents",
        GraphEdgeKind::SupportsDecision => "supports_decision",
    }
}

fn import_leaf_symbol(import_spec: &str) -> Option<&str> {
    import_spec
        .rsplit("::")
        .next()
        .and_then(|segment| segment.rsplit('.').next())
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
}

fn decision_terms_for_path(path: &str) -> BTreeSet<String> {
    path.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|term| term.len() >= 3)
        .map(str::to_ascii_lowercase)
        .collect()
}

#[derive(Debug, Default)]
struct DecisionDocContentSignals {
    has_markers: bool,
    target_hits: usize,
    impacted_overlap: usize,
}

fn scan_decision_document_content(
    path: &Path,
    target_lower: &str,
    impacted_terms: &BTreeSet<String>,
) -> std::io::Result<DecisionDocContentSignals> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut signals = DecisionDocContentSignals::default();
    let mut impacted_hits = BTreeSet::<String>::new();

    while reader.read_line(&mut line)? != 0 {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            let lowered = trimmed.to_ascii_lowercase();
            if line_contains_decision_marker(&lowered) {
                signals.has_markers = true;
            }
            if !target_lower.is_empty() && lowered.contains(target_lower) {
                signals.target_hits += 1;
            }
            mark_term_hits(impacted_terms, &mut impacted_hits, &lowered);
        }

        line.clear();
    }

    signals.impacted_overlap = impacted_hits.len();
    Ok(signals)
}

fn line_contains_decision_marker(lowered_line: &str) -> bool {
    ["decision", "rationale", "tradeoff", "status:", "intent"]
        .iter()
        .any(|marker| lowered_line.contains(marker))
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

fn looks_like_decision_doc(path: &str, language: Option<&str>, extension: Option<&str>) -> bool {
    if let Some(language) = language
        && language.eq_ignore_ascii_case("markdown")
    {
        return true;
    }
    if let Some(extension) = extension
        && extension.eq_ignore_ascii_case("md")
    {
        return true;
    }

    let lowered = path.to_ascii_lowercase();
    lowered.contains("adr")
        || lowered.contains("decision")
        || lowered.contains("intent")
        || lowered.contains("spec")
}

fn make_receipt(
    source_type: &str,
    source_ref: &str,
    locator: &str,
    captured_at: &str,
) -> EvidenceReceipt {
    EvidenceReceipt {
        id: format!(
            "ev_{}_{}_{}",
            sanitize_for_id(source_type),
            sanitize_for_id(source_ref),
            sanitize_for_id(locator)
        ),
        source_type: source_type.to_string(),
        source_ref: source_ref.to_string(),
        locator: locator.to_string(),
        snippet_hash: None,
        captured_at: captured_at.to_string(),
    }
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use repobrain_ingest::{RepositoryScanner, RepositoryTarget};

    use super::{
        BlastRadiusReport, GraphEdgeKind, GraphStore, GraphTraversalRequest, SnapshotGraphStore,
        file_node_ref, symbol_node_ref,
    };

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "repobrain-graph-{label}-{}-{}",
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
    fn blast_radius_uses_symbol_ownership_and_import_edges() {
        let repo = TempRepo::new("blast-symbol");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");
        repo.write_file(
            "docs/adr-0001-repository-scanner.md",
            b"# ADR-0001: RepositoryScanner\ndecision: keep RepositoryScanner deterministic for safe edit.\n",
        );

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let graph = SnapshotGraphStore::new(&snapshot);
        let report = graph.blast_radius("RepositoryScanner");
        let expected_symbol_node =
            symbol_node_ref(&snapshot.find_exact_symbol("RepositoryScanner")[0]);

        assert_eq!(
            report.impacted_nodes,
            vec![
                expected_symbol_node,
                file_node_ref("src/inner.rs"),
                file_node_ref("src/lib.rs"),
            ]
        );
        for edge_kind in [
            GraphEdgeKind::Defines,
            GraphEdgeKind::References,
            GraphEdgeKind::Imports,
            GraphEdgeKind::Tests,
            GraphEdgeKind::Builds,
            GraphEdgeKind::Documents,
            GraphEdgeKind::SupportsDecision,
        ] {
            assert_has_edge_kind(&report, edge_kind);
        }
        assert_eq!(
            report.verification_plan.required_checks,
            vec!["cargo test".to_string()]
        );
        assert_eq!(
            report.verification_plan.recommended_checks,
            vec!["cargo check".to_string()]
        );
        assert_eq!(report.invariants, report.verification_plan.invariants);
        assert!(
            report
                .invariants
                .iter()
                .any(|invariant| invariant.contains("symbol `RepositoryScanner` remains defined"))
        );
        assert!(
            report
                .invariants
                .iter()
                .any(|invariant| invariant.contains("required verification remains passing"))
        );
        assert_eq!(report.flow_capsules.len(), 1);
        assert!(
            report.flow_capsules[0]
                .name
                .contains("structural flow around `RepositoryScanner`")
        );
        assert_has_evidence_type(&report, "symbol_definition");
        assert_has_evidence_type(&report, "reference_edge");
        assert_has_evidence_type(&report, "decision_document");
    }

    #[test]
    fn blast_radius_includes_reverse_importers_for_path_targets() {
        let repo = TempRepo::new("blast-path");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"mod inner;\n");
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let graph = SnapshotGraphStore::new(&snapshot);
        let report = graph.blast_radius("src/inner.rs");

        assert_eq!(
            report.impacted_nodes,
            vec![file_node_ref("src/inner.rs"), file_node_ref("src/lib.rs")]
        );
        assert_eq!(
            report.verification_plan.required_checks,
            vec!["cargo test".to_string()]
        );
        assert!(report.invariants.iter().any(|invariant| {
            invariant
                .contains("anchor path `src/inner.rs` remains part of the scoped change envelope")
        }));
        assert!(
            report
                .evidence
                .iter()
                .any(|receipt| receipt.source_type == "file_anchor")
        );
    }

    #[test]
    fn traverse_returns_direct_structural_neighbors() {
        let repo = TempRepo::new("traverse");
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
        let graph = SnapshotGraphStore::new(&snapshot);
        let hits = graph.traverse(&GraphTraversalRequest {
            seed: "RepositoryScanner".to_string(),
            max_hops: 1,
            edge_kinds: vec![GraphEdgeKind::Contains, GraphEdgeKind::Imports],
        });

        assert_eq!(
            hits.iter()
                .map(|hit| hit.node_ref.clone())
                .collect::<Vec<_>>(),
            vec![
                "file:src/inner.rs".to_string(),
                "file:src/lib.rs".to_string()
            ]
        );
    }

    #[test]
    fn blast_radius_plans_repo_wide_recommended_checks_from_workspace_targets() {
        let repo = TempRepo::new("blast-recommended");
        repo.write_file(
            "Cargo.toml",
            b"[workspace]\nmembers = [\"src/rust/crates/*\", \"src/rust/xtask\"]\n",
        );
        repo.write_file(
            "src/rust/xtask/Cargo.toml",
            b"[package]\nname = \"repobrain-xtask\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/rust/crates/demo/Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/rust/crates/demo/src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\n",
        );
        repo.write_file("src/rust/crates/demo/src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let graph = SnapshotGraphStore::new(&snapshot);
        let report = graph.blast_radius("RepositoryScanner");

        assert_eq!(
            report.verification_plan.required_checks,
            vec!["cargo test (in src/rust/crates/demo)".to_string()]
        );
        assert_eq!(
            report.verification_plan.recommended_checks,
            vec![
                "cargo check (in src/rust/crates/demo)".to_string(),
                "cargo xtask check".to_string(),
                "cargo xtask quality".to_string(),
            ]
        );
        assert!(
            report.flow_capsules[0]
                .tests_covering_flow
                .contains(&"cargo test (in src/rust/crates/demo)".to_string())
        );
    }

    #[test]
    fn blast_radius_emits_stop_condition_when_no_required_checks_are_known() {
        let repo = TempRepo::new("blast-no-checks");
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
        let graph = SnapshotGraphStore::new(&snapshot);
        let report = graph.blast_radius("RepositoryScanner");

        assert!(report.verification_plan.required_checks.is_empty());
        assert_eq!(
            report.verification_plan.stop_conditions,
            vec![
                "no deterministic required checks were discovered for this blast radius; narrow scope or add manual verification"
                    .to_string()
            ]
        );
        assert_eq!(
            report.flow_capsules[0].failure_modes,
            vec![
                "no deterministic required checks were discovered for this blast radius; narrow scope or add manual verification"
                    .to_string()
            ]
        );
        assert!(!report.verification_plan.invariants.is_empty());
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => error.duration().as_nanos(),
        }
    }

    fn assert_has_edge_kind(report: &BlastRadiusReport, edge_kind: GraphEdgeKind) {
        assert!(
            report
                .relationships
                .iter()
                .any(|relationship| relationship.edge_kind == edge_kind)
        );
    }

    fn assert_has_evidence_type(report: &BlastRadiusReport, source_type: &str) {
        assert!(
            report
                .evidence
                .iter()
                .any(|receipt| receipt.source_type == source_type)
        );
    }
}
