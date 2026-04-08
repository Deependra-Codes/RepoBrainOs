use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use repobrain_domain::{
    CoverageAudit, CoverageSlot, CoverageSlotAudit, CoverageStatus, EvidenceReceipt,
    QueryClassification,
};
use repobrain_ingest::{IndexedImport, RepositoryInventorySnapshot};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RetrievalError {
    #[error("failed to read file {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("query must contain at least one alphanumeric token")]
    EmptyQuery,
    #[error("lexical cache lock is poisoned during {phase}")]
    CachePoisoned { phase: &'static str },
    #[error("failed to create lexical index directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("lexical sqlite error during {operation}: {source}")]
    Sqlite {
        operation: &'static str,
        source: rusqlite::Error,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LexicalHit {
    pub relative_path: String,
    pub score: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexicalQuery {
    pub text: String,
    pub scope: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Default)]
pub struct SnapshotLexicalIndexCache {
    snapshot_fingerprints: RwLock<BTreeMap<String, String>>,
}

impl SnapshotLexicalIndexCache {
    /// Searches a snapshot-scoped lexical index backed by a persistent FTS5
    /// store and reuses snapshot pins across requests.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot index cannot be prepared, lexical
    /// query construction fails, or SQLite operations fail.
    pub fn search(
        &self,
        snapshot: &RepositoryInventorySnapshot,
        query: &LexicalQuery,
    ) -> Result<Vec<LexicalHit>, RetrievalError> {
        self.index_for_snapshot(snapshot)?;
        search_snapshot_fts(snapshot, query)
    }

    /// Ensures a snapshot is pinned to the persistent lexical index, applying
    /// incremental changed-file refresh when needed.
    ///
    /// # Errors
    ///
    /// Returns an error when cache locking, I/O, or SQLite synchronization
    /// fails.
    pub fn index_for_snapshot(
        &self,
        snapshot: &RepositoryInventorySnapshot,
    ) -> Result<(), RetrievalError> {
        let snapshot_key = snapshot.snapshot_id.clone();
        let fingerprint = lexical_snapshot_fingerprint(snapshot);
        if self
            .snapshot_fingerprints
            .read()
            .map_err(|_| RetrievalError::CachePoisoned { phase: "read" })?
            .get(&snapshot_key)
            .is_some_and(|cached| cached == &fingerprint)
        {
            return Ok(());
        }

        let mut connection = open_index_connection(snapshot)?;
        initialize_schema(&connection)?;
        let stored_fingerprint =
            stored_snapshot_fingerprint(&connection, snapshot.snapshot_id.as_str())?;
        if stored_fingerprint.as_deref() != Some(fingerprint.as_str()) {
            sync_snapshot_documents(&mut connection, snapshot)?;
            store_snapshot_fingerprint(&connection, snapshot, &fingerprint)?;
        }

        self.snapshot_fingerprints
            .write()
            .map_err(|_| RetrievalError::CachePoisoned { phase: "write" })?
            .insert(snapshot_key, fingerprint);

        Ok(())
    }

    /// Removes a previously pinned snapshot lexical index and all persisted
    /// lexical rows for that snapshot id.
    ///
    /// # Errors
    ///
    /// Returns an error when cache locking or SQLite cleanup fails.
    pub fn invalidate_snapshot(
        &self,
        snapshot: &RepositoryInventorySnapshot,
    ) -> Result<(), RetrievalError> {
        let mut connection = open_index_connection(snapshot)?;
        initialize_schema(&connection)?;
        let transaction = connection
            .transaction()
            .map_err(|source| RetrievalError::Sqlite {
                operation: "invalidate_begin_transaction",
                source,
            })?;
        delete_snapshot_documents(&transaction, snapshot.snapshot_id.as_str())?;
        transaction
            .execute(
                "DELETE FROM snapshot_pins WHERE snapshot_id = ?1",
                params![snapshot.snapshot_id.as_str()],
            )
            .map_err(|source| RetrievalError::Sqlite {
                operation: "invalidate_delete_pin",
                source,
            })?;
        transaction
            .commit()
            .map_err(|source| RetrievalError::Sqlite {
                operation: "invalidate_commit_transaction",
                source,
            })?;

        self.snapshot_fingerprints
            .write()
            .map_err(|_| RetrievalError::CachePoisoned {
                phase: "invalidate",
            })?
            .remove(snapshot.snapshot_id.as_str());

        Ok(())
    }
}

fn search_snapshot_fts(
    snapshot: &RepositoryInventorySnapshot,
    query: &LexicalQuery,
) -> Result<Vec<LexicalHit>, RetrievalError> {
    let match_query = build_fts_match_query(query)?;
    let scope_prefix = query.scope.as_deref().map(normalize_relative_path);
    let scope_pattern = scope_prefix.as_ref().map(|prefix| format!("{prefix}%"));
    let connection = open_index_connection(snapshot)?;
    initialize_schema(&connection)?;
    let limit = lexical_query_limit(query.limit);
    let mut statement = connection
        .prepare(
            "SELECT relative_path, bm25(lexical_fts) AS rank
             FROM lexical_fts
             WHERE lexical_fts MATCH ?1
               AND snapshot_id = ?2
               AND (?3 IS NULL OR relative_path LIKE ?4)
             ORDER BY rank ASC, relative_path ASC
             LIMIT ?5",
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "search_prepare",
            source,
        })?;
    let rows = statement
        .query_map(
            params![
                match_query,
                snapshot.snapshot_id.as_str(),
                scope_prefix.as_deref(),
                scope_pattern.as_deref(),
                limit,
            ],
            |row| {
                let relative_path = row.get::<usize, String>(0)?;
                let rank = row.get::<usize, f64>(1)?;

                Ok(LexicalHit {
                    relative_path,
                    score: score_from_bm25_rank(rank),
                })
            },
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "search_query_map",
            source,
        })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|source| RetrievalError::Sqlite {
            operation: "search_collect_rows",
            source,
        })
}

fn tokenize(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| token.len() >= 2)
        .map(|token| token.to_ascii_lowercase())
        .collect::<BTreeSet<_>>()
}

fn build_fts_match_query(query: &LexicalQuery) -> Result<String, RetrievalError> {
    let tokens = tokenize(&query.text);
    if tokens.is_empty() {
        return Err(RetrievalError::EmptyQuery);
    }

    Ok(tokens
        .iter()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND "))
}

fn lexical_query_limit(limit: usize) -> i64 {
    if limit == 0 {
        i64::MAX
    } else {
        i64::try_from(limit).unwrap_or(i64::MAX)
    }
}

fn score_from_bm25_rank(rank: f64) -> f32 {
    if !rank.is_finite() || rank <= 0.0 {
        return 1.0;
    }

    (1.0_f64 / (1.0_f64 + rank)) as f32
}

fn lexical_snapshot_fingerprint(snapshot: &RepositoryInventorySnapshot) -> String {
    let mut hasher = DefaultHasher::new();
    snapshot.snapshot_id.hash(&mut hasher);
    for file in &snapshot.files {
        file.relative_path.hash(&mut hasher);
        file.content_hash.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
}

fn open_index_connection(
    snapshot: &RepositoryInventorySnapshot,
) -> Result<Connection, RetrievalError> {
    let database_path = lexical_index_db_path(snapshot);
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent).map_err(|source| RetrievalError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    Connection::open(&database_path).map_err(|source| RetrievalError::Sqlite {
        operation: "open_connection",
        source,
    })
}

fn lexical_index_db_path(snapshot: &RepositoryInventorySnapshot) -> PathBuf {
    Path::new(&snapshot.root)
        .join(".repobrain")
        .join("indexes")
        .join("lexical-fts5.sqlite3")
}

fn initialize_schema(connection: &Connection) -> Result<(), RetrievalError> {
    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS snapshot_pins (
                snapshot_id TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL,
                captured_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS indexed_files (
                snapshot_id TEXT NOT NULL,
                relative_path TEXT NOT NULL,
                content_hash TEXT NOT NULL,
                PRIMARY KEY (snapshot_id, relative_path)
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS lexical_fts USING fts5 (
                snapshot_id UNINDEXED,
                relative_path UNINDEXED,
                body,
                tokenize='unicode61 remove_diacritics 2'
            );
            ",
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "initialize_schema",
            source,
        })
}

fn stored_snapshot_fingerprint(
    connection: &Connection,
    snapshot_id: &str,
) -> Result<Option<String>, RetrievalError> {
    connection
        .query_row(
            "SELECT fingerprint FROM snapshot_pins WHERE snapshot_id = ?1",
            params![snapshot_id],
            |row| row.get::<usize, String>(0),
        )
        .optional()
        .map_err(|source| RetrievalError::Sqlite {
            operation: "stored_snapshot_fingerprint",
            source,
        })
}

fn store_snapshot_fingerprint(
    connection: &Connection,
    snapshot: &RepositoryInventorySnapshot,
    fingerprint: &str,
) -> Result<(), RetrievalError> {
    connection
        .execute(
            "INSERT INTO snapshot_pins (snapshot_id, fingerprint, captured_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(snapshot_id) DO UPDATE SET
                 fingerprint = excluded.fingerprint,
                 captured_at = excluded.captured_at",
            params![
                snapshot.snapshot_id.as_str(),
                fingerprint,
                snapshot.captured_at
            ],
        )
        .map(|_| ())
        .map_err(|source| RetrievalError::Sqlite {
            operation: "store_snapshot_fingerprint",
            source,
        })
}

fn sync_snapshot_documents(
    connection: &mut Connection,
    snapshot: &RepositoryInventorySnapshot,
) -> Result<(), RetrievalError> {
    let mut desired_hashes = BTreeMap::<String, String>::new();
    for file in &snapshot.files {
        desired_hashes.insert(
            normalize_relative_path(&file.relative_path),
            file.content_hash.clone(),
        );
    }
    let existing_hashes = load_indexed_file_hashes(connection, snapshot.snapshot_id.as_str())?;

    let mut changed_paths = Vec::new();
    for (path, hash) in &desired_hashes {
        if existing_hashes.get(path) != Some(hash) {
            changed_paths.push(path.clone());
        }
    }
    let removed_paths = existing_hashes
        .keys()
        .filter(|path| !desired_hashes.contains_key(*path))
        .cloned()
        .collect::<Vec<_>>();

    if changed_paths.is_empty() && removed_paths.is_empty() {
        return Ok(());
    }

    let mut updates = Vec::<(String, String, Option<String>)>::new();
    for path in changed_paths {
        let Some(hash) = desired_hashes.get(&path).cloned() else {
            continue;
        };
        updates.push((path.clone(), hash, read_index_body(snapshot, &path)?));
    }

    let transaction = connection
        .transaction()
        .map_err(|source| RetrievalError::Sqlite {
            operation: "sync_begin_transaction",
            source,
        })?;
    for path in removed_paths {
        delete_document(&transaction, snapshot.snapshot_id.as_str(), &path)?;
    }
    for (path, hash, body) in updates {
        delete_document(&transaction, snapshot.snapshot_id.as_str(), &path)?;
        if let Some(body) = body {
            insert_document(
                &transaction,
                snapshot.snapshot_id.as_str(),
                &path,
                &hash,
                &body,
            )?;
        }
    }
    transaction
        .commit()
        .map_err(|source| RetrievalError::Sqlite {
            operation: "sync_commit_transaction",
            source,
        })
}

fn load_indexed_file_hashes(
    connection: &Connection,
    snapshot_id: &str,
) -> Result<BTreeMap<String, String>, RetrievalError> {
    let mut statement = connection
        .prepare(
            "SELECT relative_path, content_hash
             FROM indexed_files
             WHERE snapshot_id = ?1",
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "load_indexed_file_hashes_prepare",
            source,
        })?;
    let rows = statement
        .query_map(params![snapshot_id], |row| {
            let path = row.get::<usize, String>(0)?;
            let content_hash = row.get::<usize, String>(1)?;

            Ok((path, content_hash))
        })
        .map_err(|source| RetrievalError::Sqlite {
            operation: "load_indexed_file_hashes_query_map",
            source,
        })?;
    rows.collect::<Result<BTreeMap<_, _>, _>>()
        .map_err(|source| RetrievalError::Sqlite {
            operation: "load_indexed_file_hashes_collect_rows",
            source,
        })
}

fn read_index_body(
    snapshot: &RepositoryInventorySnapshot,
    relative_path: &str,
) -> Result<Option<String>, RetrievalError> {
    let absolute_path = Path::new(&snapshot.root).join(relative_path);
    let bytes = match fs::read(&absolute_path) {
        Ok(value) => value,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(RetrievalError::Io {
                path: absolute_path,
                source: error,
            });
        }
    };
    let Ok(contents) = String::from_utf8(bytes) else {
        return Ok(None);
    };

    Ok(Some(format!("{relative_path}\n{contents}")))
}

fn delete_document(
    transaction: &Transaction<'_>,
    snapshot_id: &str,
    relative_path: &str,
) -> Result<(), RetrievalError> {
    transaction
        .execute(
            "DELETE FROM indexed_files WHERE snapshot_id = ?1 AND relative_path = ?2",
            params![snapshot_id, relative_path],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "delete_document_indexed_files",
            source,
        })?;
    transaction
        .execute(
            "DELETE FROM lexical_fts WHERE snapshot_id = ?1 AND relative_path = ?2",
            params![snapshot_id, relative_path],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "delete_document_fts",
            source,
        })?;

    Ok(())
}

fn insert_document(
    transaction: &Transaction<'_>,
    snapshot_id: &str,
    relative_path: &str,
    content_hash: &str,
    body: &str,
) -> Result<(), RetrievalError> {
    transaction
        .execute(
            "INSERT INTO indexed_files (snapshot_id, relative_path, content_hash)
             VALUES (?1, ?2, ?3)",
            params![snapshot_id, relative_path, content_hash],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "insert_document_indexed_files",
            source,
        })?;
    transaction
        .execute(
            "INSERT INTO lexical_fts (snapshot_id, relative_path, body)
             VALUES (?1, ?2, ?3)",
            params![snapshot_id, relative_path, body],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "insert_document_fts",
            source,
        })?;

    Ok(())
}

fn delete_snapshot_documents(
    transaction: &Transaction<'_>,
    snapshot_id: &str,
) -> Result<(), RetrievalError> {
    transaction
        .execute(
            "DELETE FROM indexed_files WHERE snapshot_id = ?1",
            params![snapshot_id],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "delete_snapshot_documents_indexed_files",
            source,
        })?;
    transaction
        .execute(
            "DELETE FROM lexical_fts WHERE snapshot_id = ?1",
            params![snapshot_id],
        )
        .map_err(|source| RetrievalError::Sqlite {
            operation: "delete_snapshot_documents_fts",
            source,
        })?;

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExpansionRequest {
    pub seed_paths: Vec<String>,
    pub max_hops: u8,
    pub max_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNeighbor {
    pub relative_path: String,
    pub hop_distance: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphExpansionResult {
    pub neighbors: Vec<GraphNeighbor>,
    pub evidence: Vec<EvidenceReceipt>,
}

#[must_use]
pub fn expand_import_graph(
    snapshot: &RepositoryInventorySnapshot,
    request: &GraphExpansionRequest,
) -> GraphExpansionResult {
    let mut neighbors = Vec::new();
    let mut evidence = Vec::new();
    let mut seen = BTreeSet::new();
    let mut queue = VecDeque::new();

    for seed in &request.seed_paths {
        let normalized = normalize_relative_path(seed);
        if seen.insert(normalized.clone()) {
            queue.push_back((normalized, 0_u8));
        }
    }

    while let Some((current, depth)) = queue.pop_front() {
        if depth >= request.max_hops {
            continue;
        }
        let next_depth = depth.saturating_add(1);

        for import in snapshot.direct_imports_for_path(&current) {
            if let Some(resolved_path) = import.resolved_path.as_ref() {
                let normalized = normalize_relative_path(resolved_path);
                if seen.insert(normalized.clone()) {
                    neighbors.push(GraphNeighbor {
                        relative_path: normalized.clone(),
                        hop_distance: next_depth,
                    });
                    evidence.push(reference_edge_receipt(import, &snapshot.captured_at));
                    if neighbors.len() >= request.max_candidates {
                        return GraphExpansionResult {
                            neighbors,
                            evidence,
                        };
                    }
                    queue.push_back((normalized, next_depth));
                }
            }
        }

        for import in snapshot.reverse_imports_for_path(&current) {
            let normalized = normalize_relative_path(&import.importer_path);
            if seen.insert(normalized.clone()) {
                neighbors.push(GraphNeighbor {
                    relative_path: normalized.clone(),
                    hop_distance: next_depth,
                });
                evidence.push(reference_edge_receipt(import, &snapshot.captured_at));
                if neighbors.len() >= request.max_candidates {
                    return GraphExpansionResult {
                        neighbors,
                        evidence,
                    };
                }
                queue.push_back((normalized, next_depth));
            }
        }
    }

    GraphExpansionResult {
        neighbors,
        evidence,
    }
}

fn reference_edge_receipt(import: &IndexedImport, captured_at: &str) -> EvidenceReceipt {
    EvidenceReceipt {
        id: format!("ev_reference_edge_{}", sanitize_for_id(&import.fact_id)),
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

#[must_use]
fn normalize_relative_path(value: &str) -> String {
    value.replace('\\', "/")
}

#[derive(Debug, Clone, Default)]
pub struct CoverageSignals {
    pub has_exact_anchor: bool,
    pub structural_hits: usize,
    pub flow_capsules: usize,
    pub verification_targets: usize,
    pub decision_evidence: usize,
}

#[must_use]
pub fn coverage_audit_for(
    classification: QueryClassification,
    signals: &CoverageSignals,
) -> CoverageAudit {
    let required_slots = required_slots_for(classification);
    let slot_results = required_slots
        .iter()
        .map(|slot| CoverageSlotAudit {
            slot: *slot,
            status: slot_status(*slot, signals),
            detail: slot_detail(*slot, signals),
        })
        .collect::<Vec<_>>();
    let sufficient = slot_results
        .iter()
        .all(|slot| status_is_sufficient(slot.status));
    let summary = if sufficient {
        format!(
            "Coverage audit for `{}` is sufficient.",
            classification_label(classification)
        )
    } else {
        format!(
            "Coverage audit for `{}` is incomplete.",
            classification_label(classification)
        )
    };

    CoverageAudit {
        query_classification: classification,
        required_slots,
        slot_results,
        sufficient,
        summary,
    }
}

#[must_use]
pub fn coverage_requires_targeted_second_pass(audit: &CoverageAudit) -> bool {
    audit
        .slot_results
        .iter()
        .any(|slot| slot.status == CoverageStatus::MissingRetrievable)
}

#[must_use]
pub fn status_is_sufficient(status: CoverageStatus) -> bool {
    matches!(
        status,
        CoverageStatus::Present | CoverageStatus::NotApplicable
    )
}

fn required_slots_for(classification: QueryClassification) -> Vec<CoverageSlot> {
    match classification {
        QueryClassification::EntityLookup => vec![CoverageSlot::ExactAnchor],
        QueryClassification::ArchitectureExplanation => {
            vec![CoverageSlot::StructuralContext, CoverageSlot::FlowSummary]
        }
        QueryClassification::SafeEdit
        | QueryClassification::BugFix
        | QueryClassification::FeatureImplementation => vec![
            CoverageSlot::ExactAnchor,
            CoverageSlot::StructuralContext,
            CoverageSlot::VerificationTargets,
        ],
        QueryClassification::DecisionWhy => {
            vec![CoverageSlot::ExactAnchor, CoverageSlot::DecisionEvidence]
        }
    }
}

fn slot_status(slot: CoverageSlot, signals: &CoverageSignals) -> CoverageStatus {
    match slot {
        CoverageSlot::ExactAnchor => {
            if signals.has_exact_anchor {
                CoverageStatus::Present
            } else {
                CoverageStatus::MissingRetrievable
            }
        }
        CoverageSlot::StructuralContext => {
            if signals.structural_hits > 0 {
                CoverageStatus::Present
            } else {
                CoverageStatus::MissingRetrievable
            }
        }
        CoverageSlot::FlowSummary => {
            if signals.flow_capsules > 0 {
                CoverageStatus::Present
            } else {
                CoverageStatus::MissingRetrievable
            }
        }
        CoverageSlot::VerificationTargets => {
            if signals.verification_targets > 0 {
                CoverageStatus::Present
            } else {
                CoverageStatus::NotObservable
            }
        }
        CoverageSlot::ImpactEnvelope => {
            if signals.structural_hits > 0 {
                CoverageStatus::Present
            } else {
                CoverageStatus::MissingRetrievable
            }
        }
        CoverageSlot::DecisionEvidence => {
            if signals.decision_evidence > 0 {
                CoverageStatus::Present
            } else {
                CoverageStatus::MissingRetrievable
            }
        }
    }
}

fn slot_detail(slot: CoverageSlot, signals: &CoverageSignals) -> String {
    match slot {
        CoverageSlot::ExactAnchor => {
            if signals.has_exact_anchor {
                "exact anchor present".to_string()
            } else {
                "exact anchor missing".to_string()
            }
        }
        CoverageSlot::StructuralContext => {
            if signals.structural_hits > 0 {
                format!("structural hits: {}", signals.structural_hits)
            } else {
                "no structural hits".to_string()
            }
        }
        CoverageSlot::FlowSummary => {
            if signals.flow_capsules > 0 {
                format!("flow capsules: {}", signals.flow_capsules)
            } else {
                "no flow capsules".to_string()
            }
        }
        CoverageSlot::VerificationTargets => {
            if signals.verification_targets > 0 {
                format!("verification targets: {}", signals.verification_targets)
            } else {
                "no verification targets".to_string()
            }
        }
        CoverageSlot::ImpactEnvelope => {
            if signals.structural_hits > 0 {
                format!(
                    "impact envelope based on {} structural hit(s)",
                    signals.structural_hits
                )
            } else {
                "no structural impact evidence".to_string()
            }
        }
        CoverageSlot::DecisionEvidence => {
            if signals.decision_evidence > 0 {
                format!("decision evidence count: {}", signals.decision_evidence)
            } else {
                "decision evidence missing".to_string()
            }
        }
    }
}

fn classification_label(classification: QueryClassification) -> &'static str {
    match classification {
        QueryClassification::EntityLookup => "entity_lookup",
        QueryClassification::ArchitectureExplanation => "architecture_explanation",
        QueryClassification::SafeEdit => "safe_edit",
        QueryClassification::BugFix => "bug_fix",
        QueryClassification::FeatureImplementation => "feature_implementation",
        QueryClassification::DecisionWhy => "decision_why",
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use repobrain_ingest::{RepositoryInventorySnapshot, RepositoryScanner, RepositoryTarget};
    use rusqlite::{Connection, params};

    use super::{
        CoverageSignals, GraphExpansionRequest, LexicalQuery, SnapshotLexicalIndexCache,
        coverage_audit_for, coverage_requires_targeted_second_pass, expand_import_graph,
        lexical_index_db_path,
    };
    use repobrain_domain::{CoverageSlot, CoverageStatus, QueryClassification};

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "repobrain-retrieval-{label}-{}-{}",
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

    fn scan_repo(repo: &TempRepo) -> RepositoryInventorySnapshot {
        let scanner = RepositoryScanner::default();
        scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"))
    }

    fn snapshot_pin_count(snapshot: &RepositoryInventorySnapshot) -> usize {
        let connection = Connection::open(lexical_index_db_path(snapshot))
            .unwrap_or_else(|error| panic!("failed to open lexical db: {error}"));
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM snapshot_pins WHERE snapshot_id = ?1",
                params![snapshot.snapshot_id.as_str()],
                |row| row.get::<usize, i64>(0),
            )
            .unwrap_or_else(|error| panic!("failed to count snapshot pins: {error}"));

        usize::try_from(count).unwrap_or(0)
    }

    fn indexed_file_count(snapshot: &RepositoryInventorySnapshot) -> usize {
        let connection = Connection::open(lexical_index_db_path(snapshot))
            .unwrap_or_else(|error| panic!("failed to open lexical db: {error}"));
        let count = connection
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE snapshot_id = ?1",
                params![snapshot.snapshot_id.as_str()],
                |row| row.get::<usize, i64>(0),
            )
            .unwrap_or_else(|error| panic!("failed to count indexed files: {error}"));

        usize::try_from(count).unwrap_or(0)
    }

    #[test]
    fn lexical_index_finds_scope_hits() {
        let repo = TempRepo::new("lexical");
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");
        repo.write_file("README.md", b"RepoBrain OS overview\n");
        let snapshot = scan_repo(&repo);
        let cache = SnapshotLexicalIndexCache::default();
        let hits = cache
            .search(
                &snapshot,
                &LexicalQuery {
                    text: "RepositoryScanner".to_string(),
                    scope: Some("src".to_string()),
                    limit: 4,
                },
            )
            .unwrap_or_else(|error| panic!("search failed: {error}"));

        assert!(hits.iter().any(|hit| hit.relative_path == "src/lib.rs"));
    }

    #[test]
    fn snapshot_lexical_cache_pins_snapshot_once_for_repeated_queries() {
        let repo = TempRepo::new("lexical-cache");
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");
        repo.write_file("README.md", b"RepoBrain OS overview\n");
        let snapshot = scan_repo(&repo);
        let cache = SnapshotLexicalIndexCache::default();
        cache
            .index_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("first index build failed: {error}"));
        cache
            .index_for_snapshot(&snapshot)
            .unwrap_or_else(|error| panic!("second index lookup failed: {error}"));
        let hits = cache
            .search(
                &snapshot,
                &LexicalQuery {
                    text: "RepositoryScanner".to_string(),
                    scope: Some("src".to_string()),
                    limit: 4,
                },
            )
            .unwrap_or_else(|error| panic!("cache search failed: {error}"));

        assert_eq!(snapshot_pin_count(&snapshot), 1);
        assert_eq!(indexed_file_count(&snapshot), snapshot.files.len());
        assert!(hits.iter().any(|hit| hit.relative_path == "src/lib.rs"));
    }

    #[test]
    fn lexical_cache_incrementally_refreshes_changed_files() {
        let repo = TempRepo::new("lexical-incremental");
        repo.write_file("src/lib.rs", b"pub struct LegacyAnchorToken {}\n");
        repo.write_file("README.md", b"RepoBrain OS overview\n");
        let snapshot_before = scan_repo(&repo);
        let cache = SnapshotLexicalIndexCache::default();

        let before_hits = cache
            .search(
                &snapshot_before,
                &LexicalQuery {
                    text: "LegacyAnchorToken".to_string(),
                    scope: Some("src".to_string()),
                    limit: 8,
                },
            )
            .unwrap_or_else(|error| panic!("before search failed: {error}"));
        assert!(
            before_hits
                .iter()
                .any(|hit| hit.relative_path == "src/lib.rs")
        );

        repo.write_file("src/lib.rs", b"pub struct NovelAnchorToken {}\n");
        let snapshot_after = scan_repo(&repo);
        let old_hits_after = cache
            .search(
                &snapshot_after,
                &LexicalQuery {
                    text: "LegacyAnchorToken".to_string(),
                    scope: Some("src".to_string()),
                    limit: 8,
                },
            )
            .unwrap_or_else(|error| panic!("old-term search failed: {error}"));
        let new_hits_after = cache
            .search(
                &snapshot_after,
                &LexicalQuery {
                    text: "NovelAnchorToken".to_string(),
                    scope: Some("src".to_string()),
                    limit: 8,
                },
            )
            .unwrap_or_else(|error| panic!("new-term search failed: {error}"));

        assert!(old_hits_after.is_empty());
        assert!(
            new_hits_after
                .iter()
                .any(|hit| hit.relative_path == "src/lib.rs")
        );
    }

    #[test]
    fn lexical_cache_removes_deleted_files_from_snapshot_index() {
        let repo = TempRepo::new("lexical-delete");
        repo.write_file("src/remove_me.rs", b"pub struct DeleteMeAnchor {}\n");
        repo.write_file("src/lib.rs", b"mod remove_me;\n");
        let snapshot_before = scan_repo(&repo);
        let cache = SnapshotLexicalIndexCache::default();
        let before_hits = cache
            .search(
                &snapshot_before,
                &LexicalQuery {
                    text: "DeleteMeAnchor".to_string(),
                    scope: Some("src".to_string()),
                    limit: 8,
                },
            )
            .unwrap_or_else(|error| panic!("before delete search failed: {error}"));
        assert!(
            before_hits
                .iter()
                .any(|hit| hit.relative_path == "src/remove_me.rs")
        );

        fs::remove_file(repo.root().join("src/remove_me.rs"))
            .unwrap_or_else(|error| panic!("failed to delete indexed file: {error}"));
        let snapshot_after = scan_repo(&repo);
        let after_hits = cache
            .search(
                &snapshot_after,
                &LexicalQuery {
                    text: "DeleteMeAnchor".to_string(),
                    scope: Some("src".to_string()),
                    limit: 8,
                },
            )
            .unwrap_or_else(|error| panic!("after delete search failed: {error}"));

        assert!(after_hits.is_empty());
    }

    #[test]
    fn graph_expansion_respects_hop_limit() {
        let repo = TempRepo::new("graph");
        repo.write_file("src/lib.rs", b"mod inner;\n");
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));
        let result = expand_import_graph(
            &snapshot,
            &GraphExpansionRequest {
                seed_paths: vec!["src/inner.rs".to_string()],
                max_hops: 1,
                max_candidates: 4,
            },
        );

        assert!(
            result
                .neighbors
                .iter()
                .any(|neighbor| neighbor.relative_path == "src/lib.rs")
        );
        assert!(
            result
                .neighbors
                .iter()
                .all(|neighbor| neighbor.hop_distance <= 1)
        );
    }

    #[test]
    fn coverage_audit_marks_missing_slots() {
        let signals = CoverageSignals {
            has_exact_anchor: false,
            structural_hits: 0,
            flow_capsules: 0,
            verification_targets: 0,
            decision_evidence: 0,
        };
        let audit = coverage_audit_for(QueryClassification::SafeEdit, &signals);

        assert!(!audit.sufficient);
        assert!(
            audit
                .slot_results
                .iter()
                .any(|slot| slot.slot == CoverageSlot::ExactAnchor
                    && slot.status == CoverageStatus::MissingRetrievable)
        );
        assert!(coverage_requires_targeted_second_pass(&audit));
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => error.duration().as_nanos(),
        }
    }
}
