use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use repobrain_domain::{EvidenceReceipt, canonicalize_repo_root};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

mod imports;
mod symbols;
mod syntax;
mod verification;

pub use imports::{ImportEvidenceClass, ImportKind, IndexedImport};
use imports::{extract_imports_for_file, supports_import_extraction};
pub use symbols::{IndexedSymbol, SymbolEvidenceClass, SymbolKind};
use symbols::{extract_symbols_for_file, supports_symbol_extraction};
use syntax::extract_syntax_facts_for_file;
use verification::extract_verification_targets;
pub use verification::{
    IndexedVerificationTarget, VerificationTargetKind, VerificationTargetPriority,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryTarget {
    pub root: String,
    pub revision: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionArtifact {
    pub extractor: String,
    pub output_ref: String,
    pub evidence: Vec<EvidenceReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionSnapshot {
    pub root: String,
    pub revision: Option<String>,
    pub languages: Vec<String>,
    pub artifacts: Vec<ExtractionArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedFile {
    pub relative_path: String,
    pub extension: Option<String>,
    pub language: Option<String>,
    pub size_bytes: u64,
    #[serde(default)]
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SymbolFactIndexEntry {
    fact_id: String,
    symbol_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct SymbolPathIndexEntry {
    relative_path: String,
    start: usize,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ReverseImportPathIndexEntry {
    resolved_path: String,
    start: usize,
    len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryInventorySnapshot {
    pub root: String,
    pub revision: Option<String>,
    pub snapshot_id: String,
    #[serde(default = "default_snapshot_captured_at")]
    pub captured_at: String,
    pub excluded_roots: Vec<String>,
    pub languages: Vec<String>,
    pub files: Vec<IndexedFile>,
    pub symbols: Vec<IndexedSymbol>,
    pub imports: Vec<IndexedImport>,
    #[serde(default)]
    pub verification_targets: Vec<IndexedVerificationTarget>,
    #[serde(default)]
    symbol_fact_index: Vec<SymbolFactIndexEntry>,
    #[serde(default)]
    symbol_path_index: Vec<SymbolPathIndexEntry>,
    #[serde(default)]
    symbol_path_order: Vec<usize>,
    #[serde(default)]
    reverse_import_path_index: Vec<ReverseImportPathIndexEntry>,
    #[serde(default)]
    reverse_import_order: Vec<usize>,
}

impl RepositoryInventorySnapshot {
    fn normalize(&mut self) {
        for symbol in &mut self.symbols {
            symbol.backfill_fact_id();
        }

        for import in &mut self.imports {
            import.backfill_fact_id();
        }

        self.files
            .sort_unstable_by(|left, right| left.relative_path.cmp(&right.relative_path));
        self.languages.sort_unstable();
        self.languages.dedup();
        self.symbols.sort_unstable_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
                .then_with(|| left.line_number.cmp(&right.line_number))
        });
        self.symbols.dedup();
        self.imports.sort_unstable_by(|left, right| {
            left.importer_path
                .cmp(&right.importer_path)
                .then_with(|| left.line_number.cmp(&right.line_number))
                .then_with(|| left.import_spec.cmp(&right.import_spec))
                .then_with(|| left.resolved_path.cmp(&right.resolved_path))
        });
        self.imports.dedup();
        self.verification_targets.sort_unstable_by(|left, right| {
            left.scope_root
                .cmp(&right.scope_root)
                .then_with(|| left.priority.cmp(&right.priority))
                .then_with(|| left.kind.cmp(&right.kind))
                .then_with(|| left.working_directory.cmp(&right.working_directory))
                .then_with(|| left.label.cmp(&right.label))
                .then_with(|| left.command.cmp(&right.command))
        });
        self.verification_targets.dedup();
        self.rebuild_derived_indexes();
    }

    pub fn normalize_for_persistence(&mut self) {
        self.normalize();
    }

    #[must_use]
    pub fn find_exact_path(&self, path: &str) -> Option<&IndexedFile> {
        let normalized = normalize_relative_path(path);

        // Keep the snapshot sorted so exact lookups stay O(log n) on repo-sized inventories.
        self.files
            .binary_search_by(|entry| entry.relative_path.as_str().cmp(normalized.as_str()))
            .ok()
            .map(|index| &self.files[index])
    }

    #[must_use]
    pub fn find_exact_symbol(&self, name: &str) -> &[IndexedSymbol] {
        let start = self
            .symbols
            .partition_point(|symbol| symbol.name.as_str() < name);
        let end = self
            .symbols
            .partition_point(|symbol| symbol.name.as_str() <= name);

        &self.symbols[start..end]
    }

    #[must_use]
    pub fn find_symbol_by_fact_id(&self, fact_id: &str) -> Option<&IndexedSymbol> {
        self.symbol_fact_index
            .binary_search_by(|entry| entry.fact_id.as_str().cmp(fact_id))
            .ok()
            .and_then(|index| self.symbols.get(self.symbol_fact_index[index].symbol_index))
    }

    pub fn symbols_for_path<'a>(
        &'a self,
        relative_path: &str,
    ) -> impl Iterator<Item = &'a IndexedSymbol> + 'a {
        let symbols = &self.symbols;

        self.symbol_indices_for_path(relative_path)
            .iter()
            .map(move |symbol_index| &symbols[*symbol_index])
    }

    #[must_use]
    pub fn direct_imports_for_path(&self, relative_path: &str) -> &[IndexedImport] {
        let normalized = normalize_relative_path(relative_path);
        let start = self
            .imports
            .partition_point(|entry| entry.importer_path.as_str() < normalized.as_str());
        let end = self
            .imports
            .partition_point(|entry| entry.importer_path.as_str() <= normalized.as_str());

        &self.imports[start..end]
    }

    pub fn reverse_imports_for_path<'a>(
        &'a self,
        relative_path: &str,
    ) -> impl Iterator<Item = &'a IndexedImport> + 'a {
        let imports = &self.imports;

        self.reverse_import_indices_for_path(relative_path)
            .iter()
            .map(move |import_index| &imports[*import_index])
    }

    #[must_use]
    pub fn verification_targets(&self) -> &[IndexedVerificationTarget] {
        &self.verification_targets
    }

    fn symbol_indices_for_path(&self, relative_path: &str) -> &[usize] {
        let normalized = normalize_relative_path(relative_path);
        let Some(index) = self
            .symbol_path_index
            .binary_search_by(|entry| entry.relative_path.as_str().cmp(normalized.as_str()))
            .ok()
        else {
            return &[];
        };
        let span = &self.symbol_path_index[index];

        &self.symbol_path_order[span.start..span.start + span.len]
    }

    fn reverse_import_indices_for_path(&self, relative_path: &str) -> &[usize] {
        let normalized = normalize_relative_path(relative_path);
        let Some(index) = self
            .reverse_import_path_index
            .binary_search_by(|entry| entry.resolved_path.as_str().cmp(normalized.as_str()))
            .ok()
        else {
            return &[];
        };
        let span = &self.reverse_import_path_index[index];

        &self.reverse_import_order[span.start..span.start + span.len]
    }

    fn rebuild_derived_indexes(&mut self) {
        self.rebuild_symbol_indexes();
        self.rebuild_reverse_import_indexes();
    }

    fn rebuild_symbol_indexes(&mut self) {
        self.symbol_fact_index = self
            .symbols
            .iter()
            .enumerate()
            .map(|(symbol_index, symbol)| SymbolFactIndexEntry {
                fact_id: symbol.fact_id.clone(),
                symbol_index,
            })
            .collect();
        self.symbol_fact_index
            .sort_unstable_by(|left, right| left.fact_id.cmp(&right.fact_id));

        self.symbol_path_order = (0..self.symbols.len()).collect();
        self.symbol_path_order.sort_unstable_by(|left, right| {
            let left_symbol = &self.symbols[*left];
            let right_symbol = &self.symbols[*right];

            left_symbol
                .relative_path
                .cmp(&right_symbol.relative_path)
                .then_with(|| left_symbol.line_number.cmp(&right_symbol.line_number))
                .then_with(|| left_symbol.name.cmp(&right_symbol.name))
                .then_with(|| left_symbol.fact_id.cmp(&right_symbol.fact_id))
        });

        self.symbol_path_index.clear();
        let mut start = 0;
        while start < self.symbol_path_order.len() {
            let symbol_index = self.symbol_path_order[start];
            let relative_path = self.symbols[symbol_index].relative_path.clone();
            let mut end = start + 1;

            while end < self.symbol_path_order.len()
                && self.symbols[self.symbol_path_order[end]].relative_path == relative_path
            {
                end += 1;
            }

            self.symbol_path_index.push(SymbolPathIndexEntry {
                relative_path,
                start,
                len: end - start,
            });
            start = end;
        }
    }

    fn rebuild_reverse_import_indexes(&mut self) {
        self.reverse_import_order = self
            .imports
            .iter()
            .enumerate()
            .filter_map(|(import_index, import)| {
                import.resolved_path.as_ref().map(|_| import_index)
            })
            .collect();
        self.reverse_import_order.sort_unstable_by(|left, right| {
            let left_import = &self.imports[*left];
            let right_import = &self.imports[*right];

            left_import
                .resolved_path
                .cmp(&right_import.resolved_path)
                .then_with(|| left_import.importer_path.cmp(&right_import.importer_path))
                .then_with(|| left_import.line_number.cmp(&right_import.line_number))
                .then_with(|| left_import.import_spec.cmp(&right_import.import_spec))
                .then_with(|| left_import.fact_id.cmp(&right_import.fact_id))
        });

        self.reverse_import_path_index.clear();
        let mut start = 0;
        while start < self.reverse_import_order.len() {
            let import_index = self.reverse_import_order[start];
            let resolved_path = self.imports[import_index]
                .resolved_path
                .clone()
                .unwrap_or_default();
            let mut end = start + 1;

            while end < self.reverse_import_order.len()
                && self.imports[self.reverse_import_order[end]]
                    .resolved_path
                    .as_deref()
                    == Some(resolved_path.as_str())
            {
                end += 1;
            }

            self.reverse_import_path_index
                .push(ReverseImportPathIndexEntry {
                    resolved_path,
                    start,
                    len: end - start,
                });
            start = end;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryScanner {
    excluded_directory_names: BTreeSet<&'static str>,
    excluded_file_extensions: BTreeSet<&'static str>,
    excluded_file_prefixes: Vec<&'static str>,
    max_file_bytes: u64,
}

impl Default for RepositoryScanner {
    fn default() -> Self {
        Self {
            excluded_directory_names: BTreeSet::from([
                ".git",
                ".idea",
                ".mypy_cache",
                ".repobrain",
                ".pytest_cache",
                ".ruff_cache",
                ".venv",
                ".vscode",
                "__pycache__",
                "build",
                "dist",
                "node_modules",
                "out",
                "target",
            ]),
            excluded_file_extensions: BTreeSet::from([
                "bin", "class", "dll", "dylib", "exe", "gif", "gz", "ico", "jpeg", "jpg", "pdf",
                "png", "so", "tar", "zip",
            ]),
            excluded_file_prefixes: vec![".env"],
            max_file_bytes: 2 * 1024 * 1024,
        }
    }
}

impl RepositoryScanner {
    /// Scans a repository root into a deterministic inventory snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when the repository root does not exist, cannot be
    /// canonicalized, or any directory entry cannot be read while building the
    /// snapshot.
    pub fn scan(
        &self,
        target: &RepositoryTarget,
    ) -> Result<RepositoryInventorySnapshot, IngestError> {
        let root = PathBuf::from(&target.root);
        if !root.exists() {
            return Err(IngestError::MissingRoot(target.root.clone()));
        }

        let canonical_root = canonicalize_repo_root(&root)
            .map_err(|source| IngestError::io(root.clone(), source))?;
        let mut files = Vec::new();
        let mut languages = BTreeSet::new();
        self.walk_directory(&canonical_root, &canonical_root, &mut files, &mut languages)?;
        let (symbols, imports) = collect_structural_facts(&canonical_root, &files)?;
        let verification_targets = extract_verification_targets(&canonical_root, &files)?;

        let mut snapshot = RepositoryInventorySnapshot {
            root: canonical_root.to_string_lossy().into_owned(),
            revision: target.revision.clone(),
            snapshot_id: snapshot_id(&canonical_root, target.revision.as_deref()),
            captured_at: snapshot_captured_at_now(),
            excluded_roots: self
                .excluded_directory_names
                .iter()
                .map(|name| (*name).to_string())
                .collect(),
            languages: languages.into_iter().collect(),
            files,
            symbols,
            imports,
            verification_targets,
            symbol_fact_index: Vec::new(),
            symbol_path_index: Vec::new(),
            symbol_path_order: Vec::new(),
            reverse_import_path_index: Vec::new(),
            reverse_import_order: Vec::new(),
        };
        snapshot.normalize();

        Ok(snapshot)
    }

    fn walk_directory(
        &self,
        repo_root: &Path,
        current_dir: &Path,
        files: &mut Vec<IndexedFile>,
        languages: &mut BTreeSet<String>,
    ) -> Result<(), IngestError> {
        let mut entries = fs::read_dir(current_dir)
            .map_err(|source| IngestError::io(current_dir.to_path_buf(), source))?
            .collect::<Result<Vec<_>, io::Error>>()
            .map_err(|source| IngestError::io(current_dir.to_path_buf(), source))?;
        entries.sort_unstable_by_key(fs::DirEntry::file_name);

        for entry in entries {
            let path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|source| IngestError::io(path.clone(), source))?;
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy();
            let relative_path = strip_repo_prefix(repo_root, &path);

            if file_type.is_symlink() {
                continue;
            }

            if file_type.is_dir() {
                if self.should_exclude_directory(&file_name) {
                    continue;
                }

                self.walk_directory(repo_root, &path, files, languages)?;
                continue;
            }

            if !file_type.is_file() {
                continue;
            }

            let metadata = entry
                .metadata()
                .map_err(|source| IngestError::io(path.clone(), source))?;
            if self.should_exclude_file(&file_name, path.extension(), metadata.len()) {
                continue;
            }

            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase);
            let language = extension.as_deref().and_then(language_for_extension);
            if let Some(language) = language {
                languages.insert(language.to_string());
            }
            let content_hash = file_content_hash(&path)?;

            files.push(IndexedFile {
                relative_path,
                extension,
                language: language.map(str::to_string),
                size_bytes: metadata.len(),
                content_hash,
            });
        }

        Ok(())
    }

    fn should_exclude_directory(&self, file_name: &str) -> bool {
        self.excluded_directory_names.contains(file_name)
    }

    fn should_exclude_file(
        &self,
        file_name: &str,
        extension: Option<&std::ffi::OsStr>,
        size_bytes: u64,
    ) -> bool {
        if self
            .excluded_file_prefixes
            .iter()
            .any(|prefix| file_name.starts_with(prefix))
        {
            return true;
        }

        if size_bytes > self.max_file_bytes {
            return true;
        }

        extension
            .and_then(|value| value.to_str())
            .is_some_and(|value| {
                self.excluded_file_extensions
                    .contains(&value.to_ascii_lowercase().as_str())
            })
    }
}

fn file_content_hash(path: &Path) -> Result<String, IngestError> {
    let bytes = fs::read(path).map_err(|source| IngestError::io(path.to_path_buf(), source))?;

    Ok(format!("{:016x}", fnv1a_hash(&bytes)))
}

fn fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotArtifactStore {
    repo_root: PathBuf,
}

impl SnapshotArtifactStore {
    #[must_use]
    pub fn new(repo_root: impl Into<PathBuf>) -> Self {
        Self {
            repo_root: repo_root.into(),
        }
    }

    /// Writes an inventory snapshot to the repo-local artifact directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact directory cannot be created, the
    /// snapshot cannot be serialized, or the artifact file cannot be written.
    pub fn write_inventory(
        &self,
        snapshot: &RepositoryInventorySnapshot,
    ) -> Result<PathBuf, IngestError> {
        let artifact_path = self.inventory_path(snapshot.revision.as_deref());
        let artifact_dir = self.snapshot_dir();
        fs::create_dir_all(&artifact_dir)
            .map_err(|source| IngestError::io(artifact_dir.clone(), source))?;
        let serialized =
            serde_json::to_vec_pretty(snapshot).map_err(|source| IngestError::Serialize {
                path: artifact_path.clone(),
                source,
            })?;
        fs::write(&artifact_path, serialized)
            .map_err(|source| IngestError::io(artifact_path.clone(), source))?;

        Ok(artifact_path)
    }

    /// Loads a previously written inventory snapshot from the repo-local
    /// artifact directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the artifact file does not exist, cannot be read,
    /// or cannot be deserialized into a normalized inventory snapshot.
    pub fn load_inventory(
        &self,
        revision: Option<&str>,
    ) -> Result<RepositoryInventorySnapshot, IngestError> {
        let artifact_path = self.inventory_path(revision);
        let serialized = fs::read_to_string(&artifact_path)
            .map_err(|source| IngestError::io(artifact_path.clone(), source))?;
        let mut snapshot = serde_json::from_str::<RepositoryInventorySnapshot>(&serialized)
            .map_err(|source| IngestError::Deserialize {
                path: artifact_path,
                source,
            })?;
        snapshot.normalize();

        Ok(snapshot)
    }

    #[must_use]
    pub fn inventory_path(&self, revision: Option<&str>) -> PathBuf {
        self.snapshot_dir()
            .join(format!("{}-inventory.json", artifact_label(revision)))
    }

    #[must_use]
    fn snapshot_dir(&self) -> PathBuf {
        self.repo_root.join(".repobrain").join("snapshots")
    }
}

#[derive(Debug, Error)]
pub enum IngestError {
    #[error("repository root `{0}` does not exist")]
    MissingRoot(String),
    #[error("failed to access `{path}`")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("failed to serialize snapshot artifact `{path}`")]
    Serialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to deserialize snapshot artifact `{path}`")]
    Deserialize {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

impl IngestError {
    fn io(path: PathBuf, source: io::Error) -> Self {
        Self::Io { path, source }
    }
}

pub trait Extractor {
    fn name(&self) -> &'static str;
    fn extract(&self, target: &RepositoryTarget) -> ExtractionArtifact;
}

#[must_use]
pub fn default_extraction_phases() -> &'static [&'static str] {
    &[
        "workspace_scan",
        "symbol_graph",
        "dependency_graph",
        "build_and_test_graph",
        "evidence_collection",
    ]
}

#[must_use]
pub fn exact_path_lookup<'a>(
    snapshot: &'a RepositoryInventorySnapshot,
    path: &str,
) -> Option<&'a IndexedFile> {
    snapshot.find_exact_path(path)
}

#[must_use]
pub fn exact_symbol_lookup<'a>(
    snapshot: &'a RepositoryInventorySnapshot,
    symbol_name: &str,
) -> &'a [IndexedSymbol] {
    snapshot.find_exact_symbol(symbol_name)
}

#[must_use]
pub fn symbol_fact_lookup<'a>(
    snapshot: &'a RepositoryInventorySnapshot,
    fact_id: &str,
) -> Option<&'a IndexedSymbol> {
    snapshot.find_symbol_by_fact_id(fact_id)
}

#[must_use]
pub fn direct_import_lookup<'a>(
    snapshot: &'a RepositoryInventorySnapshot,
    relative_path: &str,
) -> &'a [IndexedImport] {
    snapshot.direct_imports_for_path(relative_path)
}

pub fn reverse_import_lookup<'a>(
    snapshot: &'a RepositoryInventorySnapshot,
    relative_path: &str,
) -> impl Iterator<Item = &'a IndexedImport> + 'a {
    snapshot.reverse_imports_for_path(relative_path)
}

#[must_use]
fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/")
}

#[must_use]
fn strip_repo_prefix(repo_root: &Path, path: &Path) -> String {
    match path.strip_prefix(repo_root) {
        Ok(relative_path) => normalize_relative_path(&relative_path.to_string_lossy()),
        Err(_) => normalize_relative_path(&path.to_string_lossy()),
    }
}

#[must_use]
fn snapshot_id(repo_root: &Path, revision: Option<&str>) -> String {
    format!(
        "{}::{}",
        repo_root.to_string_lossy(),
        revision.unwrap_or("worktree")
    )
}

fn snapshot_captured_at_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| default_snapshot_captured_at())
}

fn default_snapshot_captured_at() -> String {
    "1970-01-01T00:00:00Z".to_string()
}

#[must_use]
fn artifact_label(revision: Option<&str>) -> String {
    revision.map_or_else(
        || "worktree".to_string(),
        |value| {
            value
                .chars()
                .map(|character| {
                    if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                        character
                    } else {
                        '_'
                    }
                })
                .collect()
        },
    )
}

#[must_use]
fn language_for_extension(extension: &str) -> Option<&'static str> {
    match extension {
        "js" | "jsx" => Some("javascript"),
        "json" => Some("json"),
        "md" => Some("markdown"),
        "py" => Some("python"),
        "rs" => Some("rust"),
        "toml" => Some("toml"),
        "ts" | "tsx" => Some("typescript"),
        "yaml" | "yml" => Some("yaml"),
        _ => None,
    }
}

fn read_text_contents(path: &Path) -> Result<Option<String>, IngestError> {
    let bytes = fs::read(path).map_err(|source| IngestError::io(path.to_path_buf(), source))?;

    Ok(String::from_utf8(bytes).ok())
}

fn collect_structural_facts(
    repo_root: &Path,
    files: &[IndexedFile],
) -> Result<(Vec<IndexedSymbol>, Vec<IndexedImport>), IngestError> {
    let mut symbols = Vec::new();
    let mut imports = Vec::new();
    let available_files = files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<BTreeSet<_>>();

    for file in files {
        let Some(language) = file.language.as_deref() else {
            continue;
        };
        let path = repo_root.join(&file.relative_path);
        let Some(contents) = read_text_contents(&path)? else {
            continue;
        };

        if let Some(extraction) = extract_syntax_facts_for_file(
            &file.relative_path,
            language,
            &contents,
            &available_files,
        ) {
            symbols.extend(extraction.symbols);
            imports.extend(extraction.imports);
            continue;
        }

        if supports_symbol_extraction(language) {
            symbols.extend(extract_symbols_for_file(
                &file.relative_path,
                language,
                &contents,
            ));
        }

        if supports_import_extraction(language) {
            imports.extend(extract_imports_for_file(
                &file.relative_path,
                language,
                &contents,
                &available_files,
            ));
        }
    }

    Ok((symbols, imports))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        RepositoryInventorySnapshot, RepositoryScanner, RepositoryTarget, SnapshotArtifactStore,
        SymbolKind, direct_import_lookup, exact_path_lookup, exact_symbol_lookup,
        reverse_import_lookup, symbol_fact_lookup,
    };

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "repobrain-ingest-{label}-{}-{}",
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
    fn scan_respects_default_exclusions_and_collects_languages() {
        let repo = TempRepo::new("scan");
        repo.write_file("src/lib.rs", b"pub fn value() -> u32 { 1 }\n");
        repo.write_file(
            "src/nested/module.py",
            b"class RepositoryScanner:\n    pass\n\ndef value() -> int:\n    return 1\n",
        );
        repo.write_file("docs/guide.md", b"# guide\n");
        repo.write_file(
            "Cargo.toml",
            b"[workspace]\nmembers = [\"src/rust/crates/*\"]\n",
        );
        repo.write_file(
            "src/rust/crates/demo/Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(".env", b"SECRET=1\n");
        repo.write_file(
            "node_modules/pkg/index.js",
            b"export const ignored = true;\n",
        );
        repo.write_file(".git/config", b"[core]\n");
        repo.write_file(".repobrain/snapshots/worktree-inventory.json", b"{}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = match scanner.scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: None,
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("scan failed: {error}"),
        };

        let paths = snapshot
            .files
            .iter()
            .map(|file| file.relative_path.clone())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            vec![
                "Cargo.toml".to_string(),
                "docs/guide.md".to_string(),
                "src/lib.rs".to_string(),
                "src/nested/module.py".to_string(),
                "src/rust/crates/demo/Cargo.toml".to_string()
            ]
        );
        assert_eq!(
            snapshot.languages,
            vec![
                "markdown".to_string(),
                "python".to_string(),
                "rust".to_string(),
                "toml".to_string()
            ]
        );
        assert_eq!(snapshot.symbols.len(), 3);
        assert_eq!(snapshot.imports.len(), 0);
        assert!(snapshot.captured_at.ends_with('Z'));
        assert!(
            snapshot
                .verification_targets
                .iter()
                .any(|target| target.command == vec!["cargo".to_string(), "test".to_string()])
        );
        assert!(snapshot.find_exact_path(".env").is_none());
        assert!(
            snapshot
                .find_exact_path("node_modules/pkg/index.js")
                .is_none()
        );
        assert!(
            snapshot
                .find_exact_path(".repobrain/snapshots/worktree-inventory.json")
                .is_none()
        );
    }

    #[test]
    fn snapshot_store_round_trips_inventory() {
        let repo = TempRepo::new("roundtrip");
        repo.write_file("src/lib.rs", b"pub fn value() -> u32 { 1 }\n");
        repo.write_file("README.md", b"# test\n");

        let scanner = RepositoryScanner::default();
        let snapshot = match scanner.scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: Some("abc123".to_string()),
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("scan failed: {error}"),
        };
        let store = SnapshotArtifactStore::new(repo.root());
        let artifact_path = match store.write_inventory(&snapshot) {
            Ok(path) => path,
            Err(error) => panic!("write failed: {error}"),
        };
        let loaded = match store.load_inventory(Some("abc123")) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("load failed: {error}"),
        };

        assert_eq!(loaded, snapshot);
        assert!(artifact_path.ends_with("abc123-inventory.json"));
    }

    #[test]
    fn exact_lookup_normalizes_path_separators() {
        let repo = TempRepo::new("lookup");
        repo.write_file("src/bin/main.rs", b"fn main() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = match scanner.scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: None,
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("scan failed: {error}"),
        };
        let hit = exact_path_lookup(&snapshot, r"src\bin\main.rs");

        assert!(hit.is_some());
        assert_eq!(
            hit.map(|file| file.relative_path.clone()),
            Some("src/bin/main.rs".to_string())
        );
    }

    #[test]
    fn exact_symbol_lookup_returns_all_snapshot_hits_for_a_name() {
        let repo = TempRepo::new("symbol-lookup");
        repo.write_file(
            "src/lib.rs",
            b"mod inner;\npub struct RepositoryScanner {}\npub fn scan() {}\n",
        );
        repo.write_file(
            "src/module.py",
            b"class RepositoryScanner:\n    pass\n\ndef scan() -> None:\n    return None\n",
        );
        repo.write_file(
            "src/module.ts",
            b"export interface SnapshotBinding {}\nexport function scan(): void {}\n",
        );
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = match scanner.scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: None,
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("scan failed: {error}"),
        };
        let scanner_hits = exact_symbol_lookup(&snapshot, "RepositoryScanner");
        let scan_hits = exact_symbol_lookup(&snapshot, "scan");
        let scanner_kinds = scanner_hits.iter().map(|hit| hit.kind).collect::<Vec<_>>();

        assert_eq!(scanner_hits.len(), 2);
        assert_eq!(scanner_hits[0].name, "RepositoryScanner");
        assert_eq!(scanner_kinds, vec![SymbolKind::Struct, SymbolKind::Class]);
        assert_eq!(scan_hits.len(), 3);
        assert_eq!(
            snapshot
                .symbols_for_path("src/module.py")
                .map(|symbol| symbol.name.clone())
                .collect::<Vec<_>>(),
            vec!["RepositoryScanner".to_string(), "scan".to_string()]
        );
        assert_eq!(
            symbol_fact_lookup(&snapshot, &scanner_hits[0].fact_id)
                .map(|symbol| symbol.relative_path.clone()),
            Some("src/lib.rs".to_string())
        );
    }

    #[test]
    fn direct_import_lookup_returns_structural_edges_for_a_file() {
        let repo = TempRepo::new("imports");
        repo.write_file(
            "Cargo.toml",
            b"[workspace]\nmembers = [\"src/rust/crates/*\"]\n",
        );
        repo.write_file(
            "src/rust/crates/demo/Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"mod symbols;\n");
        repo.write_file("src/symbols.rs", b"pub struct SymbolKind {}\n");

        let scanner = RepositoryScanner::default();
        let snapshot = match scanner.scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: None,
        }) {
            Ok(snapshot) => snapshot,
            Err(error) => panic!("scan failed: {error}"),
        };
        let imports = direct_import_lookup(&snapshot, "src/lib.rs");

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].resolved_path.as_deref(), Some("src/symbols.rs"));
        assert_eq!(
            reverse_import_lookup(&snapshot, "src/symbols.rs")
                .map(|import| import.importer_path.clone())
                .collect::<Vec<_>>(),
            vec!["src/lib.rs".to_string()]
        );
    }

    #[test]
    fn scan_discovers_scoped_verification_targets_from_manifests() {
        let repo = TempRepo::new("verification-targets");
        repo.write_file(
            "Cargo.toml",
            b"[workspace]\nmembers = [\"src/rust/crates/*\", \"src/rust/xtask\"]\n",
        );
        repo.write_file(
            "package.json",
            br#"{
  "name": "repobrain-os",
  "scripts": {
    "quality:check": "pnpm exec biome check ."
  }
}
"#,
        );
        repo.write_file(
            "src/rust/crates/repobrain-ingest/Cargo.toml",
            b"[package]\nname = \"repobrain-ingest\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/rust/xtask/Cargo.toml",
            b"[package]\nname = \"repobrain-xtask\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/ts/packages/mcp-server/package.json",
            br#"{
  "name": "@repobrain/mcp-server",
  "scripts": {
    "typecheck": "tsc -p tsconfig.json --noEmit"
  }
}
"#,
        );
        repo.write_file(
            "src/python/repobrain_research/pyproject.toml",
            br#"[project]
name = "repobrain-research"

[tool.ruff]
line-length = 100

[tool.mypy]
python_version = "3.12"
"#,
        );
        repo.write_file(
            "src/python/repobrain_research/tests/test_demo.py",
            b"def test_demo() -> None:\n    assert True\n",
        );
        repo.write_file(
            "src/python/repobrain_research/src/demo.py",
            b"def demo() -> None:\n    return None\n",
        );

        let scanner = RepositoryScanner::default();
        let snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: None,
            })
            .unwrap_or_else(|error| panic!("scan failed: {error}"));

        let commands = snapshot
            .verification_targets()
            .iter()
            .map(|target| (target.working_directory.clone(), target.command.clone()))
            .collect::<Vec<_>>();

        assert!(commands.contains(&(
            "src/rust/crates/repobrain-ingest".to_string(),
            vec!["cargo".to_string(), "test".to_string()]
        )));
        assert!(commands.contains(&(
            ".".to_string(),
            vec![
                "cargo".to_string(),
                "xtask".to_string(),
                "quality".to_string()
            ]
        )));
        assert!(commands.contains(&(
            "src/ts/packages/mcp-server".to_string(),
            vec!["pnpm".to_string(), "typecheck".to_string()]
        )));
        assert!(commands.contains(&(
            "src/python/repobrain_research".to_string(),
            vec![
                "python".to_string(),
                "-m".to_string(),
                "unittest".to_string(),
                "discover".to_string(),
                "tests".to_string()
            ]
        )));
    }

    #[test]
    fn legacy_snapshot_without_verification_targets_still_deserializes() {
        let serialized = r#"{
  "root": "D:/RepoBrainOS",
  "revision": null,
  "snapshot_id": "D:/RepoBrainOS::worktree",
  "excluded_roots": [],
  "languages": [],
  "files": [],
  "symbols": [],
  "imports": []
}"#;
        let snapshot = serde_json::from_str::<RepositoryInventorySnapshot>(serialized)
            .unwrap_or_else(|error| panic!("failed to deserialize legacy snapshot: {error}"));

        assert_eq!(snapshot.captured_at, "1970-01-01T00:00:00Z");
        assert!(snapshot.verification_targets.is_empty());
    }

    #[test]
    fn load_inventory_backfills_missing_fact_ids_and_lookup_indexes() {
        let repo = TempRepo::new("legacy-fact-ids");
        let store = SnapshotArtifactStore::new(repo.root());
        repo.write_file(
            ".repobrain/snapshots/worktree-inventory.json",
            br#"{
  "root": "D:/RepoBrainOS",
  "revision": null,
  "snapshot_id": "D:/RepoBrainOS::worktree",
  "captured_at": "1970-01-01T00:00:00Z",
  "excluded_roots": [],
  "languages": ["rust"],
  "files": [],
  "symbols": [
    {
      "name": "RepositoryScanner",
      "kind": "struct",
      "relative_path": "src/lib.rs",
      "language": "rust",
      "line_number": 2,
      "evidence_class": "syntax_confirmed"
    }
  ],
  "imports": [
    {
      "importer_path": "src/lib.rs",
      "import_spec": "crate::symbols::SymbolKind",
      "line_number": 3,
      "language": "rust",
      "kind": "use",
      "resolved_path": "src/symbols.rs",
      "evidence_class": "syntax_confirmed"
    }
  ],
  "verification_targets": []
}"#,
        );

        let snapshot = store
            .load_inventory(None)
            .unwrap_or_else(|error| panic!("load failed: {error}"));

        assert!(!snapshot.symbols[0].fact_id.is_empty());
        assert!(!snapshot.imports[0].fact_id.is_empty());
        assert_eq!(
            symbol_fact_lookup(&snapshot, &snapshot.symbols[0].fact_id)
                .map(|symbol| symbol.name.clone()),
            Some("RepositoryScanner".to_string())
        );
        assert_eq!(
            snapshot
                .symbols_for_path("src/lib.rs")
                .map(|symbol| symbol.name.clone())
                .collect::<Vec<_>>(),
            vec!["RepositoryScanner".to_string()]
        );
        assert_eq!(
            reverse_import_lookup(&snapshot, "src/symbols.rs")
                .map(|import| import.importer_path.clone())
                .collect::<Vec<_>>(),
            vec!["src/lib.rs".to_string()]
        );
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => error.duration().as_nanos(),
        }
    }
}
