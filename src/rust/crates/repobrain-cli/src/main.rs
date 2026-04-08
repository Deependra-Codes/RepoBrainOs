use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::process::{Command as ProcessCommand, Output, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{ArgAction, Args, Parser, Subcommand, ValueEnum};
use repobrain_broker::{SnapshotBrokerInput, SnapshotContextBroker};
use repobrain_domain::{
    ConsumerType, ContextRequest, EquivalenceContract, EquivalenceEvidenceReceipt,
    EquivalenceEvidenceStatus, EquivalenceStage, FreshnessRequirement, ModelClass, ModelProfile,
    OverlayClaimScope, OverlayKind, RequestDepth, ScaffoldingLevel, TaskType, TheoremContract,
    TheoremObligationStatus, TheoremProofCertificate, TheoremProofObligation, TheoremRunStatus,
    canonicalize_repo_root,
};
use repobrain_graph::{FlowCapsule, GraphStore, SnapshotGraphStore};
use repobrain_ingest::{
    IngestError, RepositoryInventorySnapshot, RepositoryScanner, RepositoryTarget,
    SnapshotArtifactStore, SymbolEvidenceClass, SymbolKind, exact_path_lookup, exact_symbol_lookup,
    symbol_fact_lookup,
};
use repobrain_serving::{
    FreshnessStatus, ReadinessAssessment, ServingHealth, ServingMetadataInput, SnapshotKey,
    SnapshotStatus, build_serving_metadata,
};
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "repobrain")]
#[command(about = "RepoBrain OS workspace helper")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Doctor,
    Layout,
    SchemaIndex,
    Scan(ScanArgs),
    ScanDelta(ScanDeltaArgs),
    GetBrief(GetBriefArgs),
    LookupPath(LookupPathArgs),
    LookupSymbol(LookupSymbolArgs),
    BlastRadius(BlastRadiusArgs),
    ExplainFlow(ExplainFlowArgs),
    ListInvariants(ListInvariantsArgs),
    WhatChangedSemantically(Box<WhatChangedSemanticallyArgs>),
    ReplayTheorem(ReplayTheoremArgs),
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTaskType {
    RepoOnboarding,
    SafeEdit,
    ExplainFlow,
    BlastRadius,
    SemanticDiff,
    Query,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliFlowDepth {
    Compact,
    Standard,
    Deep,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSemanticDiffStage {
    L0StructuralDelta,
    L1RustBounded,
    L2RelationalSemantic,
    L3TheoremContract,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliL1KaniMode {
    Probe,
    Check,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliL2AlignmentMode {
    ExactOnly,
    SignatureAware,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTheoremSolverMode {
    RelationalHeuristic,
    SmtZ3,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTranslationIrMode {
    PreferCompilerIr,
    RequireCompilerIr,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTranslationRunStatusMode {
    SidecarOnly,
    Override,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOverlayKind {
    None,
    Worktree,
    Buffer,
    Mixed,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliOverlayClaimScope {
    SnapshotConfirmed,
    OverlayAdjusted,
    OverlayLocalOnly,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliSnapshotStatus {
    Missing,
    Warming,
    Available,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliFreshnessStatus {
    Current,
    Stale,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliServingHealth {
    Normal,
    Degraded,
    BulkRefresh,
    OverlayLocalOnly,
}

#[derive(Debug, Clone, Args)]
struct CliServingArgs {
    #[arg(long, value_enum, default_value_t = CliOverlayKind::None)]
    overlay_kind: CliOverlayKind,
    #[arg(long, value_enum, default_value_t = CliOverlayClaimScope::SnapshotConfirmed)]
    claim_scope: CliOverlayClaimScope,
    #[arg(long)]
    overlay_hash: Option<String>,
    #[arg(long = "touched-path")]
    touched_paths: Vec<String>,
    #[arg(long, value_enum, default_value_t = CliSnapshotStatus::Available)]
    snapshot_status: CliSnapshotStatus,
    #[arg(long, value_enum, default_value_t = CliFreshnessStatus::Current)]
    freshness_status: CliFreshnessStatus,
    #[arg(long, value_enum, default_value_t = CliServingHealth::Normal)]
    serving_health: CliServingHealth,
}

#[derive(Debug, Clone)]
struct ServingRuntimeInput {
    readiness_assessment: ReadinessAssessment,
    overlay_kind: OverlayKind,
    claim_scope: OverlayClaimScope,
    overlay_hash: Option<String>,
    touched_paths: Vec<String>,
}

#[derive(Debug, Args)]
struct ScanArgs {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct ScanDeltaArgs {
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[arg(long, value_name = "REF", default_value = "worktree")]
    base_revision: String,
    #[arg(long = "changed-path")]
    changed_paths: Vec<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct GetBriefArgs {
    #[arg(long)]
    goal: String,
    #[arg(long)]
    scope: String,
    #[arg(long)]
    token_budget: u32,
    #[arg(long, value_enum, default_value_t = CliTaskType::SafeEdit)]
    task_type: CliTaskType,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct LookupPathArgs {
    path: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct LookupSymbolArgs {
    symbol: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct BlastRadiusArgs {
    target: String,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct ExplainFlowArgs {
    #[arg(long)]
    flow_name: String,
    #[arg(long, value_enum, default_value_t = CliFlowDepth::Standard)]
    depth: CliFlowDepth,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct ListInvariantsArgs {
    #[arg(long)]
    scope: String,
    #[arg(long, default_value_t = 4096)]
    token_budget: u32,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct WhatChangedSemanticallyArgs {
    #[arg(long, value_name = "REF")]
    r#ref: String,
    #[arg(long, value_enum, default_value_t = CliSemanticDiffStage::L0StructuralDelta)]
    stage: CliSemanticDiffStage,
    #[arg(long, value_enum, default_value_t = CliL1KaniMode::Probe)]
    l1_kani_mode: CliL1KaniMode,
    #[arg(long, default_value_t = 8)]
    l1_max_rust_files: usize,
    #[arg(long, default_value_t = 16)]
    l1_max_functions: usize,
    #[arg(long, default_value_t = 15_000)]
    l1_timeout_ms: u64,
    #[arg(long, value_enum, default_value_t = CliL2AlignmentMode::SignatureAware)]
    l2_alignment_mode: CliL2AlignmentMode,
    #[arg(long, default_value_t = 24)]
    l2_max_pairs: usize,
    #[arg(long, default_value_t = 96)]
    l2_max_candidate_functions: usize,
    #[arg(long, default_value_t = 15_000)]
    l2_timeout_ms: u64,
    #[arg(long, default_value_t = 65)]
    l2_min_alignment_score: u8,
    #[arg(long)]
    theorem_contract_file: Option<PathBuf>,
    #[arg(long, default_value_t = 128)]
    theorem_max_obligations: usize,
    #[arg(long, default_value_t = 256)]
    theorem_max_candidate_functions: usize,
    #[arg(long, default_value_t = 30_000)]
    theorem_timeout_ms: u64,
    #[arg(long, default_value_t = 1)]
    theorem_flaky_retries: u8,
    #[arg(long, default_value_t = 1)]
    theorem_stability_runs: u8,
    #[arg(long, default_value_t = 1)]
    theorem_max_parallelism: usize,
    #[arg(long, value_enum, default_value_t = CliTheoremSolverMode::SmtZ3)]
    theorem_solver_mode: CliTheoremSolverMode,
    #[arg(long)]
    theorem_solver_path: Option<PathBuf>,
    #[arg(long, default_value_t = 10_000)]
    theorem_solver_timeout_ms: u64,
    #[arg(long, default_value_t = true)]
    theorem_solver_fallback_relational: bool,
    #[arg(long)]
    theorem_replay_out: Option<PathBuf>,
    #[arg(long, default_value_t = true)]
    theorem_persist_replay: bool,
    #[arg(long, default_value_t = true, action = ArgAction::Set)]
    theorem_translation_validation: bool,
    #[arg(long)]
    theorem_alive2_path: Option<PathBuf>,
    #[arg(long, default_value_t = 32)]
    theorem_translation_max_obligations: usize,
    #[arg(long, default_value_t = 8_000)]
    theorem_translation_timeout_ms: u64,
    #[arg(long, value_enum, default_value_t = CliTranslationIrMode::RequireCompilerIr)]
    theorem_translation_ir_mode: CliTranslationIrMode,
    #[arg(long, value_enum, default_value_t = CliTranslationRunStatusMode::Override)]
    theorem_translation_run_status_mode: CliTranslationRunStatusMode,
    #[arg(long)]
    repo_root: Option<PathBuf>,
    #[arg(long)]
    revision: Option<String>,
    #[command(flatten)]
    serving: CliServingArgs,
}

#[derive(Debug, Args)]
struct ReplayTheoremArgs {
    #[arg(long)]
    artifact: PathBuf,
    #[arg(long)]
    obligation_id: Option<String>,
}

const MAX_DELTA_SCOPE_ITEMS: usize = 96;
const MAX_DELTA_SCOPE_PREVIEW_ITEMS: usize = 8;
const MAX_UNRESOLVED_CHANGED_PATH_PREVIEW: usize = 8;
const MAX_DIRECTORY_EXPANSION_PREVIEW: usize = 6;

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Doctor => {
            doctor();
            Ok(())
        }
        Command::Layout => {
            layout();
            Ok(())
        }
        Command::SchemaIndex => {
            schema_index();
            Ok(())
        }
        Command::Scan(args) => scan(args),
        Command::ScanDelta(args) => scan_delta(args),
        Command::GetBrief(args) => get_brief(args),
        Command::LookupPath(args) => lookup_path(args),
        Command::LookupSymbol(args) => lookup_symbol(args),
        Command::BlastRadius(args) => blast_radius(args),
        Command::ExplainFlow(args) => explain_flow(args),
        Command::ListInvariants(args) => list_invariants(args),
        Command::WhatChangedSemantically(args) => what_changed_semantically(*args),
        Command::ReplayTheorem(args) => replay_theorem(args),
    }
}

fn doctor() {
    println!("RepoBrain OS workspace doctor");
    println!("- Rust core: src/rust/crates");
    println!("- Python research: src/python/repobrain_research");
    println!("- TypeScript integrations: src/ts/packages");
    println!("- Canonical schemas: schemas");
}

fn layout() {
    println!("RepoBrain layout");
    println!("docs/");
    println!("research/");
    println!("schemas/");
    println!("scripts/");
    println!("src/rust/crates/");
    println!("src/python/repobrain_research/");
    println!("src/ts/packages/");
}

fn schema_index() {
    println!("schemas/model-profile.schema.json");
    println!("schemas/evidence-receipt.schema.json");
    println!("schemas/context-request.schema.json");
    println!("schemas/briefing-pack.schema.json");
    println!("schemas/impact-summary.schema.json");
    println!("schemas/theorem-contract.schema.json");
    println!("schemas/theorem-proof-certificate.schema.json");
    println!("schemas/theorem-replay-artifact.schema.json");
}

fn scan(args: ScanArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let scanner = RepositoryScanner::default();
    let snapshot = scanner
        .scan(&RepositoryTarget {
            root: repo_root.to_string_lossy().into_owned(),
            revision: args.revision.clone(),
        })
        .context("failed to build repository inventory snapshot")?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let previous_snapshot = load_previous_snapshot_for_scan(&store, args.revision.as_deref());
    let artifact_path = store
        .write_inventory(&snapshot)
        .context("failed to persist repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);

    println!("snapshot: {}", metadata.snapshot_binding.snapshot_id);
    println!("readiness: {}", readiness_label(metadata.readiness_state));
    println!("artifact: {}", artifact_path.display());
    println!("files: {}", snapshot.files.len());
    println!("symbols: {}", snapshot.symbols.len());
    println!("imports: {}", snapshot.imports.len());
    println!(
        "verification_targets: {}",
        snapshot.verification_targets.len()
    );
    println!("languages: {}", snapshot.languages.join(", "));
    match previous_snapshot {
        Some(previous_snapshot) => {
            println!("previous_snapshot: {}", previous_snapshot.snapshot_id);
            let delta = semantic_delta(&previous_snapshot, &snapshot, MAX_DELTA_SCOPE_ITEMS);
            print_semantic_delta_summary("snapshot_diff", &delta);
        }
        None => {
            println!("previous_snapshot: none (first scan for this revision label)");
        }
    }

    Ok(())
}

fn scan_delta(args: ScanDeltaArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let normalized_changed_inputs = normalize_changed_paths(&repo_root, &args.changed_paths)?;
    if normalized_changed_inputs.is_empty() {
        bail!("scan-delta requires at least one --changed-path entry");
    }

    let scanner = RepositoryScanner::default();
    let target_snapshot = scanner
        .scan(&RepositoryTarget {
            root: repo_root.to_string_lossy().into_owned(),
            revision: args.revision.clone(),
        })
        .context("failed to build target inventory snapshot for delta ingest")?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let base_revision = revision_label_to_option(&args.base_revision);
    let base_snapshot = store
        .load_inventory(base_revision)
        .with_context(|| {
            format!(
                "failed to load base snapshot `{}`; run `repobrain scan --revision {} --repo-root {}` first",
                args.base_revision,
                args.base_revision,
                repo_root.display()
            )
        })?;
    let changed_resolution = resolve_changed_paths_for_delta(
        &normalized_changed_inputs,
        &base_snapshot,
        &target_snapshot,
    );
    if changed_resolution.resolved_paths.is_empty() {
        bail!(
            "scan-delta could not resolve any --changed-path entries against base `{}` and target `{}` snapshots. unresolved inputs: {}",
            base_snapshot.snapshot_id,
            target_snapshot.snapshot_id,
            preview_paths(
                &changed_resolution.unresolved_inputs,
                MAX_UNRESOLVED_CHANGED_PATH_PREVIEW
            )
        );
    }

    let merged_snapshot = merge_delta_snapshot(
        &base_snapshot,
        &target_snapshot,
        &changed_resolution.resolved_paths,
    );
    let artifact_path = store
        .write_inventory(&merged_snapshot)
        .context("failed to persist delta-merged repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let applied_delta = semantic_delta(&base_snapshot, &merged_snapshot, MAX_DELTA_SCOPE_ITEMS);
    let full_target_delta = semantic_delta(&base_snapshot, &target_snapshot, MAX_DELTA_SCOPE_ITEMS);

    println!("snapshot: {}", metadata.snapshot_binding.snapshot_id);
    println!("base_snapshot: {}", base_snapshot.snapshot_id);
    println!("target_snapshot: {}", target_snapshot.snapshot_id);
    println!("readiness: {}", readiness_label(metadata.readiness_state));
    println!("artifact: {}", artifact_path.display());
    print_changed_path_resolution_summary(normalized_changed_inputs.len(), &changed_resolution);
    println!("files: {}", merged_snapshot.files.len());
    println!("symbols: {}", merged_snapshot.symbols.len());
    println!("imports: {}", merged_snapshot.imports.len());
    println!(
        "verification_targets: {}",
        merged_snapshot.verification_targets.len()
    );
    println!("languages: {}", merged_snapshot.languages.join(", "));
    print_semantic_delta_summary("applied_delta", &applied_delta);
    print_semantic_delta_summary("full_target_delta", &full_target_delta);
    print_delta_scope_coverage(&applied_delta, &full_target_delta);

    Ok(())
}

fn revision_label_to_option(value: &str) -> Option<&str> {
    if value.eq_ignore_ascii_case("worktree") {
        None
    } else {
        Some(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct ChangedPathResolution {
    resolved_paths: BTreeSet<String>,
    unresolved_inputs: Vec<String>,
    directory_expansions: Vec<(String, usize)>,
}

fn load_previous_snapshot_for_scan(
    store: &SnapshotArtifactStore,
    revision: Option<&str>,
) -> Option<RepositoryInventorySnapshot> {
    match store.load_inventory(revision) {
        Ok(snapshot) => Some(snapshot),
        Err(IngestError::Io { source, .. }) if source.kind() == ErrorKind::NotFound => None,
        Err(error) => {
            println!("warning: previous snapshot could not be loaded for diff summary: {error}");
            None
        }
    }
}

fn normalize_changed_paths(repo_root: &Path, changed_paths: &[String]) -> Result<BTreeSet<String>> {
    let mut normalized_paths = BTreeSet::new();

    for path in changed_paths {
        if let Some(normalized) = normalize_changed_path(repo_root, path)? {
            let _ = normalized_paths.insert(normalized);
        }
    }

    Ok(normalized_paths)
}

fn normalize_changed_path(repo_root: &Path, path: &str) -> Result<Option<String>> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let candidate = PathBuf::from(trimmed);
    let normalized = if candidate.is_absolute() {
        match candidate.strip_prefix(repo_root) {
            Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
            Err(_) => {
                bail!(
                    "--changed-path `{trimmed}` is outside repo root `{}`",
                    repo_root.display()
                );
            }
        }
    } else {
        trimmed.replace('\\', "/")
    };
    if contains_parent_dir_segment(&normalized) {
        bail!("--changed-path `{trimmed}` contains parent traversal `..`");
    }

    let normalized = normalized
        .trim()
        .trim_start_matches("./")
        .trim_matches('/')
        .to_string();
    if normalized.is_empty() {
        return Ok(None);
    }

    Ok(Some(normalized))
}

fn contains_parent_dir_segment(path: &str) -> bool {
    Path::new(path)
        .components()
        .any(|component| matches!(component, Component::ParentDir))
}

fn resolve_changed_paths_for_delta(
    normalized_inputs: &BTreeSet<String>,
    base_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
) -> ChangedPathResolution {
    let known_paths = base_snapshot
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .chain(
            target_snapshot
                .files
                .iter()
                .map(|file| file.relative_path.clone()),
        )
        .collect::<BTreeSet<_>>();
    let mut resolution = ChangedPathResolution::default();

    for input in normalized_inputs {
        if known_paths.contains(input) {
            let _ = resolution.resolved_paths.insert(input.clone());
            continue;
        }

        let matches = collect_prefixed_paths(&known_paths, input);
        if matches.is_empty() {
            resolution.unresolved_inputs.push(input.clone());
            continue;
        }

        resolution
            .directory_expansions
            .push((input.clone(), matches.len()));
        resolution.resolved_paths.extend(matches);
    }

    resolution
}

fn collect_prefixed_paths(known_paths: &BTreeSet<String>, prefix: &str) -> Vec<String> {
    let normalized_prefix = prefix.trim_end_matches('/');
    if normalized_prefix.is_empty() {
        return Vec::new();
    }
    let prefix_with_separator = format!("{normalized_prefix}/");

    known_paths
        .range(prefix_with_separator.clone()..)
        .take_while(|path| path.starts_with(&prefix_with_separator))
        .cloned()
        .collect()
}

fn print_changed_path_resolution_summary(input_count: usize, resolution: &ChangedPathResolution) {
    let resolved_input_count = input_count.saturating_sub(resolution.unresolved_inputs.len());
    let resolved_input_percent = ratio_percent(resolved_input_count, input_count);

    println!("changed_path_inputs: {input_count}");
    println!(
        "changed_path_resolution: {resolved_input_count}/{input_count} ({resolved_input_percent}%)"
    );
    println!("changed_paths: {}", resolution.resolved_paths.len());
    if !resolution.directory_expansions.is_empty() {
        println!(
            "changed_path_directory_expansions: {}",
            resolution.directory_expansions.len()
        );
        for (input, expanded_file_count) in resolution
            .directory_expansions
            .iter()
            .take(MAX_DIRECTORY_EXPANSION_PREVIEW)
        {
            println!("  {input} -> {expanded_file_count} file(s)");
        }
    }
    if !resolution.unresolved_inputs.is_empty() {
        println!(
            "warning_unresolved_changed_paths: {}",
            resolution.unresolved_inputs.len()
        );
        println!(
            "warning_unresolved_preview: {}",
            preview_paths(
                &resolution.unresolved_inputs,
                MAX_UNRESOLVED_CHANGED_PATH_PREVIEW
            )
        );
        println!(
            "warning_hint: unresolved paths are ignored; use repo-relative file paths or directory prefixes"
        );
    }
}

fn print_semantic_delta_summary(label: &str, delta: &SemanticDelta) {
    println!(
        "{label}: files +{} -{} ~{}, symbols +{} -{}, imports +{} -{}, verification_targets +{} -{}",
        delta.added_files.len(),
        delta.removed_files.len(),
        delta.modified_files.len(),
        delta.added_symbol_facts,
        delta.removed_symbol_facts,
        delta.added_import_facts,
        delta.removed_import_facts,
        delta.added_verification_targets.len(),
        delta.removed_verification_targets.len(),
    );
    println!("{label}_changed_scope: {}", delta.changed_scope.len());
    if !delta.changed_scope.is_empty() {
        println!(
            "{label}_scope_preview: {}",
            preview_paths(&delta.changed_scope, MAX_DELTA_SCOPE_PREVIEW_ITEMS)
        );
    }
}

fn print_delta_scope_coverage(applied_delta: &SemanticDelta, full_target_delta: &SemanticDelta) {
    let applied_scope = applied_delta
        .changed_scope
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let full_scope = full_target_delta
        .changed_scope
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let matched_scope_count = applied_scope.intersection(&full_scope).count();
    let full_scope_count = full_scope.len();
    let coverage_percent = ratio_percent(matched_scope_count, full_scope_count);
    let missing_scope = full_scope
        .difference(&applied_scope)
        .cloned()
        .collect::<Vec<_>>();

    println!(
        "delta_scope_coverage: {matched_scope_count}/{full_scope_count} ({coverage_percent}%)"
    );
    if !missing_scope.is_empty() {
        println!("delta_scope_gap_count: {}", missing_scope.len());
        println!(
            "delta_scope_gap_preview: {}",
            preview_paths(&missing_scope, MAX_DELTA_SCOPE_PREVIEW_ITEMS)
        );
        println!(
            "delta_scope_hint: widen --changed-path or run `repobrain scan` when bounded delta excludes expected scope"
        );
    }
}

fn ratio_percent(numerator: usize, denominator: usize) -> usize {
    if denominator == 0 {
        return 100;
    }

    numerator.saturating_mul(100) / denominator
}

fn preview_paths(paths: &[String], limit: usize) -> String {
    if paths.is_empty() {
        return "none".to_string();
    }
    let mut preview = paths.iter().take(limit).cloned().collect::<Vec<_>>();
    if paths.len() > limit {
        preview.push(format!("... +{}", paths.len() - limit));
    }

    preview.join(", ")
}

fn merge_delta_snapshot(
    base_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
    changed_paths: &BTreeSet<String>,
) -> RepositoryInventorySnapshot {
    let mut merged = base_snapshot.clone();
    merged.root.clone_from(&target_snapshot.root);
    merged.revision.clone_from(&target_snapshot.revision);
    merged.snapshot_id.clone_from(&target_snapshot.snapshot_id);
    merged.captured_at.clone_from(&target_snapshot.captured_at);
    merged
        .excluded_roots
        .clone_from(&target_snapshot.excluded_roots);

    merged.files = merge_delta_files(&base_snapshot.files, &target_snapshot.files, changed_paths);
    merged.symbols = merge_delta_symbols(
        &base_snapshot.symbols,
        &target_snapshot.symbols,
        changed_paths,
    );
    merged.imports = merge_delta_imports(
        &base_snapshot.imports,
        &target_snapshot.imports,
        changed_paths,
    );
    merged.verification_targets = if changed_paths_affect_verification(changed_paths) {
        target_snapshot.verification_targets.clone()
    } else {
        base_snapshot.verification_targets.clone()
    };
    merged.languages = merged
        .files
        .iter()
        .filter_map(|file| file.language.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    merged.normalize_for_persistence();

    merged
}

fn merge_delta_files(
    base_files: &[repobrain_ingest::IndexedFile],
    target_files: &[repobrain_ingest::IndexedFile],
    changed_paths: &BTreeSet<String>,
) -> Vec<repobrain_ingest::IndexedFile> {
    let target_by_path = target_files
        .iter()
        .map(|file| (file.relative_path.clone(), file.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut merged = base_files
        .iter()
        .filter(|file| !changed_paths.contains(&file.relative_path))
        .cloned()
        .collect::<Vec<_>>();
    merged.extend(
        changed_paths
            .iter()
            .filter_map(|path| target_by_path.get(path))
            .cloned(),
    );
    merged
}

fn merge_delta_symbols(
    base_symbols: &[repobrain_ingest::IndexedSymbol],
    target_symbols: &[repobrain_ingest::IndexedSymbol],
    changed_paths: &BTreeSet<String>,
) -> Vec<repobrain_ingest::IndexedSymbol> {
    let mut merged = base_symbols
        .iter()
        .filter(|symbol| !changed_paths.contains(&symbol.relative_path))
        .cloned()
        .collect::<Vec<_>>();
    merged.extend(
        target_symbols
            .iter()
            .filter(|symbol| changed_paths.contains(&symbol.relative_path))
            .cloned(),
    );
    merged
}

fn merge_delta_imports(
    base_imports: &[repobrain_ingest::IndexedImport],
    target_imports: &[repobrain_ingest::IndexedImport],
    changed_paths: &BTreeSet<String>,
) -> Vec<repobrain_ingest::IndexedImport> {
    let mut importer_paths = changed_paths.clone();

    for import in base_imports.iter().chain(target_imports) {
        if import
            .resolved_path
            .as_ref()
            .is_some_and(|path| changed_paths.contains(path))
        {
            let _ = importer_paths.insert(import.importer_path.clone());
        }
    }

    let mut merged = base_imports
        .iter()
        .filter(|import| !importer_paths.contains(&import.importer_path))
        .filter(|import| {
            import
                .resolved_path
                .as_ref()
                .is_none_or(|path| !changed_paths.contains(path))
        })
        .cloned()
        .collect::<Vec<_>>();
    merged.extend(
        target_imports
            .iter()
            .filter(|import| importer_paths.contains(&import.importer_path))
            .cloned(),
    );

    merged
}

fn changed_paths_affect_verification(changed_paths: &BTreeSet<String>) -> bool {
    changed_paths.iter().any(|path| {
        let lowered = path.to_ascii_lowercase();
        lowered.ends_with("/cargo.toml")
            || lowered == "cargo.toml"
            || lowered.ends_with("/package.json")
            || lowered == "package.json"
            || lowered.ends_with("/pyproject.toml")
            || lowered == "pyproject.toml"
            || lowered.ends_with("/requirements.txt")
            || lowered == "requirements.txt"
            || lowered.ends_with("/setup.py")
            || lowered == "setup.py"
            || lowered.ends_with("/pnpm-workspace.yaml")
            || lowered == "pnpm-workspace.yaml"
            || lowered.contains("/tests/")
            || lowered.starts_with("tests/")
    })
}

fn get_brief(args: GetBriefArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let broker = SnapshotContextBroker::default();
    let briefing = broker
        .get_brief(SnapshotBrokerInput {
            request: ContextRequest {
                goal: args.goal.clone(),
                task_type: args.task_type.into(),
                consumer_type: ConsumerType::Cli,
                model_profile: default_model_profile(),
                question: args.goal,
                scope_hint: Some(args.scope),
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
        .context("failed to compile snapshot-backed briefing pack")?;

    println!(
        "{}",
        serde_json::to_string_pretty(&briefing)
            .context("failed to serialize briefing pack as json")?
    );

    Ok(())
}

fn blast_radius(args: BlastRadiusArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let graph = SnapshotGraphStore::new(&snapshot);
    let report = graph.blast_radius(&args.target);
    if report.impacted_nodes.is_empty() {
        bail!(
            "target `{}` was not found in snapshot {}",
            args.target,
            metadata.snapshot_binding.snapshot_id
        );
    }

    println!("snapshot: {}", metadata.snapshot_binding.snapshot_id);
    println!("readiness: {}", readiness_label(metadata.readiness_state));
    println!("impacted_nodes: {}", report.impacted_nodes.len());

    for node in &report.impacted_nodes {
        println!("impact: {}", render_impact_node(&snapshot, node));
    }

    if !report.relationships.is_empty() {
        println!("relationships: {}", report.relationships.len());
        for relationship in &report.relationships {
            println!("  {}", render_relationship_line(&snapshot, relationship));
        }
    }

    if !report.verification_plan.required_checks.is_empty() {
        println!("required_checks:");
        for target in &report.verification_plan.required_checks {
            println!("  {target}");
        }
    }

    if !report.verification_plan.recommended_checks.is_empty() {
        println!("recommended_checks:");
        for target in &report.verification_plan.recommended_checks {
            println!("  {target}");
        }
    }

    if !report.verification_plan.coverage_gaps.is_empty() {
        println!("coverage_gaps:");
        for gap in &report.verification_plan.coverage_gaps {
            println!("  {gap}");
        }
    }

    if !report.verification_plan.stop_conditions.is_empty() {
        println!("stop_conditions:");
        for condition in &report.verification_plan.stop_conditions {
            println!("  {condition}");
        }
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct FlowTrace {
    name: String,
    entrypoints: Vec<String>,
    core_modules: Vec<String>,
    state_transitions: Vec<String>,
    failure_modes: Vec<String>,
    tests_covering_flow: Vec<String>,
    evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ExplainFlowOutput {
    snapshot_id: String,
    readiness_state: String,
    flow_name: String,
    depth: String,
    impacted_nodes: usize,
    relationships: usize,
    flow_capsules: Vec<FlowTrace>,
    verification_targets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ListInvariantsOutput {
    snapshot_id: String,
    readiness_state: String,
    scope: String,
    invariants: Vec<String>,
    do_not_break: Vec<String>,
    required_checks: Vec<String>,
    recommended_checks: Vec<String>,
    coverage_gaps: Vec<String>,
    stop_conditions: Vec<String>,
    coverage_summary: String,
}

mod semantic_pipeline;
use semantic_pipeline::{
    SemanticDelta, explain_flow, list_invariants, replay_theorem, semantic_delta,
    what_changed_semantically,
};

#[cfg(test)]
use semantic_pipeline::{
    L1RustBoundedScope, L2RelationalPolicy, TheoremPolicy, TheoremReplayArtifact,
    is_rust_source_path, l1_collect_proof_obligations, l1_rust_bounded_scope, l1_status_label,
    l2_relational_semantic_receipt, l2_status_label, load_theorem_contract_for_stage,
    theorem_obligation_for_source_pair, theorem_schedule_obligations, theorem_stage_artifacts,
    theorem_translation_validation_receipt,
};

fn lookup_path(args: LookupPathArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let Some(hit) = exact_path_lookup(&snapshot, &args.path) else {
        bail!(
            "path `{}` was not found in snapshot {}",
            args.path,
            metadata.snapshot_binding.snapshot_id
        );
    };

    println!("snapshot: {}", metadata.snapshot_binding.snapshot_id);
    println!("readiness: {}", readiness_label(metadata.readiness_state));
    println!("path: {}", hit.relative_path);
    println!("size_bytes: {}", hit.size_bytes);
    println!("language: {}", hit.language.as_deref().unwrap_or("unknown"));

    Ok(())
}

fn lookup_symbol(args: LookupSymbolArgs) -> Result<()> {
    let serving = serving_runtime_input(&args.serving);
    let repo_root = resolve_repo_root(args.repo_root)?;
    let store = SnapshotArtifactStore::new(&repo_root);
    let snapshot = store
        .load_inventory(args.revision.as_deref())
        .context("failed to load stored repository inventory snapshot")?;
    let metadata = serving_metadata(&repo_root, args.revision, &serving);
    let hits = exact_symbol_lookup(&snapshot, &args.symbol);
    if hits.is_empty() {
        bail!(
            "symbol `{}` was not found in snapshot {}",
            args.symbol,
            metadata.snapshot_binding.snapshot_id
        );
    }

    println!("snapshot: {}", metadata.snapshot_binding.snapshot_id);
    println!("readiness: {}", readiness_label(metadata.readiness_state));
    println!("matches: {}", hits.len());

    for hit in hits {
        println!(
            "symbol: {} kind: {} path: {}:{} evidence: {}",
            hit.name,
            symbol_kind_label(hit.kind),
            hit.relative_path,
            hit.line_number,
            symbol_evidence_label(hit.evidence_class)
        );
    }

    Ok(())
}

fn resolve_repo_root(repo_root: Option<PathBuf>) -> Result<PathBuf> {
    let repo_root = match repo_root {
        Some(repo_root) => repo_root,
        None => env::current_dir().context("failed to resolve current working directory")?,
    };

    let canonical_root = canonicalize_repo_root(&repo_root)
        .with_context(|| format!("failed to canonicalize repo root {}", repo_root.display()))?;

    Ok(canonical_root)
}

fn default_model_profile() -> ModelProfile {
    ModelProfile {
        id: "frontier-cli".to_string(),
        class: ModelClass::FrontierAgent,
        max_context_tokens: 128_000,
        preferred_scaffolding_level: ScaffoldingLevel::Minimal,
        notes: Vec::new(),
    }
}

fn serving_runtime_input(args: &CliServingArgs) -> ServingRuntimeInput {
    ServingRuntimeInput {
        readiness_assessment: ReadinessAssessment {
            snapshot_status: args.snapshot_status.into(),
            freshness_status: args.freshness_status.into(),
            serving_health: args.serving_health.into(),
        },
        overlay_kind: args.overlay_kind.into(),
        claim_scope: args.claim_scope.into(),
        overlay_hash: args.overlay_hash.clone(),
        touched_paths: args.touched_paths.clone(),
    }
}

fn serving_metadata(
    repo_root: &Path,
    revision: Option<String>,
    serving: &ServingRuntimeInput,
) -> repobrain_serving::ServingMetadata {
    let snapshot = SnapshotKey::new(
        repo_root.to_string_lossy().into_owned(),
        revision,
        serving.overlay_hash.clone(),
    );

    build_serving_metadata(
        &snapshot,
        ServingMetadataInput {
            readiness_assessment: serving.readiness_assessment.clone(),
            overlay_kind: serving.overlay_kind,
            claim_scope: serving.claim_scope,
            overlay_hash: serving.overlay_hash.clone(),
            touched_paths: serving.touched_paths.clone(),
            required_checks: Vec::new(),
            recommended_checks: Vec::new(),
            invariants: Vec::new(),
            coverage_gaps: Vec::new(),
            stop_conditions: Vec::new(),
        },
    )
}

fn readiness_label(state: repobrain_domain::ReadinessState) -> &'static str {
    match state {
        repobrain_domain::ReadinessState::Cold => "cold",
        repobrain_domain::ReadinessState::Warming => "warming",
        repobrain_domain::ReadinessState::Ready => "ready",
        repobrain_domain::ReadinessState::Stale => "stale",
        repobrain_domain::ReadinessState::Degraded => "degraded",
        repobrain_domain::ReadinessState::BulkRefresh => "bulk_refresh",
        repobrain_domain::ReadinessState::OverlayOnly => "overlay_only",
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

fn symbol_evidence_label(evidence: SymbolEvidenceClass) -> &'static str {
    match evidence {
        SymbolEvidenceClass::LexicalConfirmed => "lexical_confirmed",
        SymbolEvidenceClass::SyntaxConfirmed => "syntax_confirmed",
    }
}

fn flow_trace(capsule: &FlowCapsule, depth: CliFlowDepth) -> FlowTrace {
    let limits = match depth {
        CliFlowDepth::Compact => (2, 3, 2, 2, 2, 2),
        CliFlowDepth::Standard => (4, 6, 6, 4, 4, 6),
        CliFlowDepth::Deep => (
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ),
    };

    FlowTrace {
        name: capsule.name.clone(),
        entrypoints: take_prefix(&capsule.entrypoints, limits.0),
        core_modules: take_prefix(&capsule.core_modules, limits.1),
        state_transitions: take_prefix(&capsule.state_transitions, limits.2),
        failure_modes: take_prefix(&capsule.failure_modes, limits.3),
        tests_covering_flow: take_prefix(&capsule.tests_covering_flow, limits.4),
        evidence_ids: capsule
            .evidence
            .iter()
            .take(limits.5)
            .map(|receipt| receipt.id.clone())
            .collect(),
    }
}

fn cli_flow_depth_label(depth: CliFlowDepth) -> &'static str {
    match depth {
        CliFlowDepth::Compact => "compact",
        CliFlowDepth::Standard => "standard",
        CliFlowDepth::Deep => "deep",
    }
}

fn take_prefix(values: &[String], max_len: usize) -> Vec<String> {
    values.iter().take(max_len).cloned().collect()
}

fn merged_invariants(briefing: &repobrain_domain::BriefingPack) -> Vec<String> {
    let mut merged = briefing.do_not_break.clone();
    for invariant in &briefing.verification_plan.invariants {
        if !merged.contains(invariant) {
            merged.push(invariant.clone());
        }
    }

    merged
}

fn render_impact_node(
    snapshot: &repobrain_ingest::RepositoryInventorySnapshot,
    node_ref: &str,
) -> String {
    if let Some(fact_id) = node_ref.strip_prefix("symbol:")
        && let Some(symbol) = symbol_fact_lookup(snapshot, fact_id)
    {
        return format!(
            "symbol:{}@{} [{}]",
            symbol.name, symbol.relative_path, symbol.fact_id
        );
    }

    node_ref.to_string()
}

fn render_relationship_line(
    snapshot: &repobrain_ingest::RepositoryInventorySnapshot,
    relationship: &repobrain_graph::GraphRelationship,
) -> String {
    format!(
        "{}: {} -> {}",
        relationship_kind_label(relationship.edge_kind),
        render_impact_node(snapshot, &relationship.source_ref),
        render_impact_node(snapshot, &relationship.target_ref)
    )
}

fn relationship_kind_label(kind: repobrain_graph::GraphEdgeKind) -> &'static str {
    match kind {
        repobrain_graph::GraphEdgeKind::Contains => "contains",
        repobrain_graph::GraphEdgeKind::Defines => "defines",
        repobrain_graph::GraphEdgeKind::Imports => "imports",
        repobrain_graph::GraphEdgeKind::References => "references",
        repobrain_graph::GraphEdgeKind::Calls => "calls",
        repobrain_graph::GraphEdgeKind::Builds => "builds",
        repobrain_graph::GraphEdgeKind::Tests => "tests",
        repobrain_graph::GraphEdgeKind::Documents => "documents",
        repobrain_graph::GraphEdgeKind::SupportsDecision => "supports_decision",
    }
}

impl From<CliTaskType> for TaskType {
    fn from(value: CliTaskType) -> Self {
        match value {
            CliTaskType::RepoOnboarding => TaskType::RepoOnboarding,
            CliTaskType::SafeEdit => TaskType::SafeEdit,
            CliTaskType::ExplainFlow => TaskType::ExplainFlow,
            CliTaskType::BlastRadius => TaskType::BlastRadius,
            CliTaskType::SemanticDiff => TaskType::SemanticDiff,
            CliTaskType::Query => TaskType::Query,
        }
    }
}

impl From<CliOverlayKind> for OverlayKind {
    fn from(value: CliOverlayKind) -> Self {
        match value {
            CliOverlayKind::None => OverlayKind::None,
            CliOverlayKind::Worktree => OverlayKind::Worktree,
            CliOverlayKind::Buffer => OverlayKind::Buffer,
            CliOverlayKind::Mixed => OverlayKind::Mixed,
        }
    }
}

impl From<CliOverlayClaimScope> for OverlayClaimScope {
    fn from(value: CliOverlayClaimScope) -> Self {
        match value {
            CliOverlayClaimScope::SnapshotConfirmed => OverlayClaimScope::SnapshotConfirmed,
            CliOverlayClaimScope::OverlayAdjusted => OverlayClaimScope::OverlayAdjusted,
            CliOverlayClaimScope::OverlayLocalOnly => OverlayClaimScope::OverlayLocalOnly,
        }
    }
}

impl From<CliSnapshotStatus> for SnapshotStatus {
    fn from(value: CliSnapshotStatus) -> Self {
        match value {
            CliSnapshotStatus::Missing => SnapshotStatus::Missing,
            CliSnapshotStatus::Warming => SnapshotStatus::Warming,
            CliSnapshotStatus::Available => SnapshotStatus::Available,
        }
    }
}

impl From<CliFreshnessStatus> for FreshnessStatus {
    fn from(value: CliFreshnessStatus) -> Self {
        match value {
            CliFreshnessStatus::Current => FreshnessStatus::Current,
            CliFreshnessStatus::Stale => FreshnessStatus::Stale,
        }
    }
}

impl From<CliServingHealth> for ServingHealth {
    fn from(value: CliServingHealth) -> Self {
        match value {
            CliServingHealth::Normal => ServingHealth::Normal,
            CliServingHealth::Degraded => ServingHealth::Degraded,
            CliServingHealth::BulkRefresh => ServingHealth::BulkRefresh,
            CliServingHealth::OverlayLocalOnly => ServingHealth::OverlayLocalOnly,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    use repobrain_domain::{TheoremContract, TheoremObligationStatus, TheoremRunStatus};
    use repobrain_ingest::{RepositoryScanner, RepositoryTarget};

    use super::{
        CliFreshnessStatus, CliL1KaniMode, CliL2AlignmentMode, CliOverlayClaimScope,
        CliOverlayKind, CliSemanticDiffStage, CliServingArgs, CliServingHealth, CliSnapshotStatus,
        CliTheoremSolverMode, CliTranslationIrMode, CliTranslationRunStatusMode,
        EquivalenceEvidenceStatus, L1RustBoundedScope, L2RelationalPolicy, TheoremPolicy,
        TheoremReplayArtifact, WhatChangedSemanticallyArgs, changed_paths_affect_verification,
        is_rust_source_path, l1_collect_proof_obligations, l1_rust_bounded_scope, l1_status_label,
        l2_relational_semantic_receipt, l2_status_label, load_theorem_contract_for_stage,
        merge_delta_snapshot, normalize_changed_paths, resolve_changed_paths_for_delta,
        semantic_delta, theorem_obligation_for_source_pair, theorem_schedule_obligations,
        theorem_stage_artifacts, theorem_translation_validation_receipt,
    };

    struct TempRepo {
        path: PathBuf,
    }

    impl TempRepo {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!(
                "repobrain-cli-{label}-{}-{}",
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
    fn semantic_delta_reports_changes_between_reference_and_target_snapshots() {
        let repo = TempRepo::new("semantic-delta-changed");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file(
            "src/lib.rs",
            b"mod new_mod;\npub struct RepositoryScanner {\n    enabled: bool,\n}\n",
        );
        repo.write_file("src/new_mod.rs", b"pub fn helper() {}\n");

        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);

        assert!(delta.has_changes());
        assert!(delta.added_files.contains(&"src/new_mod.rs".to_string()));
        assert!(delta.modified_files.contains(&"src/lib.rs".to_string()));
        assert!(delta.added_symbol_facts > 0);
        assert!(delta.added_import_facts > 0);
        assert!(delta.changed_scope.contains(&"src/new_mod.rs".to_string()));
    }

    #[test]
    fn semantic_delta_reports_no_change_for_identical_snapshots() {
        let repo = TempRepo::new("semantic-delta-unchanged");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub struct RepositoryScanner {}\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);

        assert!(!delta.has_changes());
        assert!(delta.changed_scope.is_empty());
        assert!(delta.added_files.is_empty());
        assert!(delta.removed_files.is_empty());
        assert!(delta.modified_files.is_empty());
        assert_eq!(delta.added_symbol_facts, 0);
        assert_eq!(delta.removed_symbol_facts, 0);
        assert_eq!(delta.added_import_facts, 0);
        assert_eq!(delta.removed_import_facts, 0);
        assert!(delta.added_verification_targets.is_empty());
        assert!(delta.removed_verification_targets.is_empty());
    }

    #[test]
    fn l1_scope_is_bounded_and_rust_only() {
        let repo = TempRepo::new("l1-scope-bounds");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn alpha() {}\npub fn beta() {}\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file(
            "src/lib.rs",
            b"pub fn alpha() {}\npub fn beta() {}\npub fn gamma() {}\n",
        );
        repo.write_file("docs/readme.md", b"# docs\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let scope = l1_rust_bounded_scope(&target_snapshot, &delta, 1, 2);

        assert!(scope.rust_files.len() <= 1);
        assert!(scope.function_symbols.len() <= 2);
        assert!(
            scope
                .rust_files
                .iter()
                .all(|path| is_rust_source_path(path))
        );
    }

    #[test]
    fn l1_collect_proof_obligations_discovers_kani_harnesses() {
        let repo = TempRepo::new("l1-proof-obligation-discovery");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file(
            "src/lib.rs",
            b"#[kani::proof]\nfn proof_addition_commutes() {\n    assert_eq!(1 + 1, 2);\n}\n",
        );
        let scope = L1RustBoundedScope {
            rust_files: vec!["src/lib.rs".to_string()],
            function_symbols: Vec::new(),
        };

        let obligations = l1_collect_proof_obligations(repo.root(), &scope, 8);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].harness_name, "proof_addition_commutes");
        assert_eq!(obligations[0].manifest_path, "Cargo.toml");
        assert_eq!(obligations[0].source_path, "src/lib.rs");
    }

    #[test]
    fn l1_status_label_maps_evidence_statuses() {
        assert_eq!(
            l1_status_label(EquivalenceEvidenceStatus::NoDifferenceObserved),
            "l1_bounded_no_counterexample"
        );
        assert_eq!(
            l1_status_label(EquivalenceEvidenceStatus::ObservedDifference),
            "l1_bounded_counterexample"
        );
        assert_eq!(
            l1_status_label(EquivalenceEvidenceStatus::Inconclusive),
            "l1_bounded_inconclusive"
        );
    }

    #[test]
    fn l2_status_label_maps_evidence_statuses() {
        assert_eq!(
            l2_status_label(EquivalenceEvidenceStatus::NoDifferenceObserved),
            "l2_relational_no_counterexample"
        );
        assert_eq!(
            l2_status_label(EquivalenceEvidenceStatus::ObservedDifference),
            "l2_relational_counterexample"
        );
        assert_eq!(
            l2_status_label(EquivalenceEvidenceStatus::Inconclusive),
            "l2_relational_inconclusive"
        );
    }

    #[test]
    fn l2_relational_receipt_aligns_renamed_function_with_no_difference_signal() {
        let repo = TempRepo::new("l2-relational-rename");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn calculate_total() -> i32 { 1 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let receipt = l2_relational_semantic_receipt(
            &reference_snapshot,
            &target_snapshot,
            &delta,
            l2_test_policy(),
        );

        assert_eq!(
            receipt.status,
            EquivalenceEvidenceStatus::NoDifferenceObserved
        );
        assert_eq!(
            receipt.stage,
            repobrain_domain::EquivalenceStage::L2RelationalSemantic
        );
    }

    #[test]
    fn l2_relational_receipt_detects_import_context_difference() {
        let repo = TempRepo::new("l2-relational-import-change");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/helper.rs", b"pub fn helper_value() -> i32 { 7 }\n");
        repo.write_file(
            "src/lib.rs",
            b"mod helper;\npub fn compute_total() -> i32 { 1 }\n",
        );

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file(
            "src/lib.rs",
            b"mod helper;\nuse crate::helper::helper_value;\npub fn compute_total() -> i32 { helper_value() }\n",
        );
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let receipt = l2_relational_semantic_receipt(
            &reference_snapshot,
            &target_snapshot,
            &delta,
            l2_test_policy(),
        );

        assert_eq!(
            receipt.status,
            EquivalenceEvidenceStatus::ObservedDifference
        );
        assert!(
            receipt
                .witness
                .as_deref()
                .is_some_and(|value| value.contains("l2_pair_diff"))
        );
    }

    #[test]
    fn l2_relational_receipt_detects_function_body_difference() {
        let repo = TempRepo::new("l2-relational-body-change");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 2 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let receipt = l2_relational_semantic_receipt(
            &reference_snapshot,
            &target_snapshot,
            &delta,
            l2_test_policy(),
        );

        assert_eq!(
            receipt.status,
            EquivalenceEvidenceStatus::ObservedDifference
        );
        assert!(receipt.witness.as_deref().is_some_and(|value| {
            value.contains("literal_signature")
                || value.contains("window_hash")
                || value.contains("modified_file_without_semantic_signal")
        }));
    }

    #[test]
    fn l2_relational_receipt_aligns_moved_function_with_same_behavior() {
        let repo = TempRepo::new("l2-relational-move-rename");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"mod new_logic;\n");
        repo.write_file(
            "src/new_logic.rs",
            b"pub fn calculate_total() -> i32 { 1 }\n",
        );
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let receipt = l2_relational_semantic_receipt(
            &reference_snapshot,
            &target_snapshot,
            &delta,
            l2_test_policy(),
        );

        assert_eq!(
            receipt.status,
            EquivalenceEvidenceStatus::NoDifferenceObserved
        );
    }

    #[test]
    fn theorem_stage_requires_explicit_contract_file() {
        let args = theorem_test_args(None);

        let result =
            load_theorem_contract_for_stage(&args, CliSemanticDiffStage::L3TheoremContract);

        assert!(result.is_err());
    }

    #[test]
    fn theorem_contract_file_loads_and_validates() {
        let repo = TempRepo::new("theorem-contract-load");
        repo.write_file(
            "contract.json",
            br#"{
  "contract_id":"demo-contract",
  "observable_outputs":["stdout"],
  "error_behavior":["exit codes"],
  "side_effects":["none"],
  "preconditions":["same inputs"],
  "environment_assumptions":["stable filesystem"]
}"#,
        );
        let args = theorem_test_args(Some(repo.root().join("contract.json")));

        let contract =
            load_theorem_contract_for_stage(&args, CliSemanticDiffStage::L3TheoremContract)
                .unwrap_or_else(|error| panic!("failed to load contract: {error}"));
        let Some(contract) = contract else {
            panic!("contract should be loaded")
        };

        assert_eq!(contract.contract_id, "demo-contract");
        assert_eq!(contract.observable_outputs, vec!["stdout".to_string()]);
    }

    #[test]
    fn theorem_stage_artifacts_are_deterministic() {
        let repo = TempRepo::new("theorem-obligation-determinism");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 2 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let contract = theorem_test_contract();
        let policy = TheoremPolicy {
            max_obligations: 16,
            max_candidate_functions: 32,
            timeout_ms: 2_000,
            flaky_retries: 1,
            stability_runs: 1,
            max_parallelism: 1,
            solver_mode: CliTheoremSolverMode::SmtZ3,
            solver_path: None,
            solver_timeout_ms: 10_000,
            solver_fallback_relational: true,
            replay_out: None,
            persist_replay: false,
            translation_validation: false,
            alive2_path: None,
            translation_max_obligations: 32,
            translation_timeout_ms: 1_000,
            translation_ir_mode: CliTranslationIrMode::RequireCompilerIr,
            translation_run_status_mode: CliTranslationRunStatusMode::Override,
        };

        let first = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &policy,
            &contract,
        );
        let second = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &policy,
            &contract,
        );

        assert_eq!(first.obligations, second.obligations);
        assert_eq!(first.certificates, second.certificates);
        assert_eq!(first.run_status, second.run_status);
    }

    #[test]
    fn theorem_stage_artifacts_can_prove_aligned_pairs() {
        let repo = TempRepo::new("theorem-proved-aligned-pair");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn calculate_total() -> i32 { 1 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let artifacts = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &theorem_test_policy(),
            &theorem_test_contract(),
        );

        assert_eq!(
            artifacts.run_status,
            Some(TheoremRunStatus::ProvedUnderContract)
        );
        assert!(
            artifacts
                .certificates
                .iter()
                .all(|certificate| matches!(certificate.status, TheoremObligationStatus::Proved))
        );
    }

    #[test]
    fn theorem_stage_artifacts_translation_override_can_degrade_run_status() {
        let repo = TempRepo::new("theorem-proved-pair-with-translation-override");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn calculate_total() -> i32 { 1 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let mut policy = theorem_test_policy();
        policy.translation_validation = true;
        policy.alive2_path = Some(PathBuf::from("definitely-missing-alive2-binary"));
        policy.translation_ir_mode = CliTranslationIrMode::RequireCompilerIr;
        policy.translation_run_status_mode = CliTranslationRunStatusMode::Override;
        let artifacts = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &policy,
            &theorem_test_contract(),
        );

        assert_eq!(
            artifacts.run_status,
            Some(TheoremRunStatus::InconclusiveUnderContract)
        );
        assert!(artifacts.extra_receipts.iter().any(|receipt| {
            receipt.backend == "alive2_translation_validation"
                && matches!(receipt.status, EquivalenceEvidenceStatus::Inconclusive)
        }));
    }

    #[test]
    fn theorem_stage_artifacts_can_disable_translation_run_status_override() {
        let repo = TempRepo::new("theorem-proved-pair-without-translation-override");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn calculate_total() -> i32 { 1 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let mut policy = theorem_test_policy();
        policy.translation_validation = true;
        policy.alive2_path = Some(PathBuf::from("definitely-missing-alive2-binary"));
        policy.translation_ir_mode = CliTranslationIrMode::RequireCompilerIr;
        policy.translation_run_status_mode = CliTranslationRunStatusMode::SidecarOnly;
        let artifacts = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &policy,
            &theorem_test_contract(),
        );

        assert_eq!(
            artifacts.run_status,
            Some(TheoremRunStatus::ProvedUnderContract)
        );
    }

    #[test]
    fn theorem_stage_artifacts_persist_replay_artifact() {
        let repo = TempRepo::new("theorem-replay-artifact");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let reference_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("reference scan failed: {error}"));

        repo.write_file("src/lib.rs", b"pub fn compute_total() -> i32 { 2 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let replay_path = repo
            .root()
            .join(".repobrain")
            .join("theorem-runs")
            .join("run.json");
        let mut policy = theorem_test_policy();
        policy.replay_out = Some(replay_path.clone());
        policy.persist_replay = true;

        let delta = semantic_delta(&reference_snapshot, &target_snapshot, 32);
        let artifacts = theorem_stage_artifacts(
            repo.root(),
            &reference_snapshot,
            &target_snapshot,
            &delta,
            &policy,
            &theorem_test_contract(),
        );

        assert!(artifacts.replay_artifact.is_some());
        let serialized = fs::read_to_string(&replay_path)
            .unwrap_or_else(|error| panic!("failed reading replay artifact: {error}"));
        let replay: TheoremReplayArtifact = serde_json::from_str(&serialized)
            .unwrap_or_else(|error| panic!("failed parsing replay artifact: {error}"));
        assert_eq!(replay.contract_id, "demo-contract");
        assert_eq!(replay.obligations, artifacts.obligations);
        assert_eq!(replay.certificates, artifacts.certificates);
    }

    #[test]
    fn theorem_schedule_orders_by_priority_and_partition() {
        let contract = theorem_test_contract();
        let obligations = vec![
            theorem_obligation_for_source_pair(
                "file:src/lib.rs".to_string(),
                Vec::new(),
                &contract,
            ),
            theorem_obligation_for_source_pair(
                "pair:beta@src/b.rs:1=>beta@src/b.rs:1".to_string(),
                Vec::new(),
                &contract,
            ),
            theorem_obligation_for_source_pair(
                "reference_only:alpha@src/a.rs:1".to_string(),
                Vec::new(),
                &contract,
            ),
        ];
        let scheduled = theorem_schedule_obligations(obligations, &theorem_test_policy());
        let ordered_sources = scheduled
            .iter()
            .map(|obligation| obligation.source_pair.clone())
            .collect::<Vec<_>>();

        assert_eq!(ordered_sources[0], "reference_only:alpha@src/a.rs:1");
        assert!(ordered_sources[1].starts_with("pair:"));
        assert_eq!(ordered_sources[2], "file:src/lib.rs");
    }

    #[test]
    fn theorem_translation_sidecar_is_inconclusive_when_no_pair_obligations_exist() {
        let repo = TempRepo::new("theorem-translation-sidecar");
        let mut policy = theorem_test_policy();
        policy.translation_validation = true;
        policy.alive2_path = Some(PathBuf::from("definitely-missing-alive2-binary"));
        let receipt = theorem_translation_validation_receipt(repo.root(), &[], &policy)
            .unwrap_or_else(|| panic!("translation sidecar receipt should be present"));

        assert_eq!(receipt.backend, "alive2_translation_validation");
        assert_eq!(receipt.status, EquivalenceEvidenceStatus::Inconclusive);
        assert!(
            receipt
                .witness
                .as_deref()
                .is_some_and(|value| { value.contains("alive2_no_pair_obligations") })
        );
    }

    #[test]
    fn theorem_translation_sidecar_reports_tool_unavailable_receipt() {
        let repo = TempRepo::new("theorem-translation-counters");
        let mut policy = theorem_test_policy();
        policy.translation_validation = true;
        policy.translation_max_obligations = 1;
        policy.alive2_path = Some(PathBuf::from("definitely-missing-alive2-binary"));
        let obligation = theorem_obligation_for_source_pair(
            "pair:alpha@src/lib.rs:10=>alpha@src/lib.rs:10".to_string(),
            Vec::new(),
            &theorem_test_contract(),
        );
        let receipt = theorem_translation_validation_receipt(repo.root(), &[obligation], &policy)
            .unwrap_or_else(|| {
                panic!("translation sidecar receipt should be present");
            });

        let bounds = receipt.bounds.unwrap_or_default();
        assert!(bounds.contains("checked_obligations=0"));
        assert!(receipt.assumptions.iter().any(|assumption| {
            assumption.contains("configured Alive2 binary could not be started")
        }));
        assert!(
            receipt
                .witness
                .as_deref()
                .is_some_and(|witness| witness.contains("alive2_tool_unavailable"))
        );
    }

    #[test]
    fn merge_delta_snapshot_refreshes_only_changed_paths() {
        let repo = TempRepo::new("scan-delta-merge");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"mod inner;\npub fn stable() {}\n");
        repo.write_file("src/inner.rs", b"pub fn helper() -> i32 { 1 }\n");

        let scanner = RepositoryScanner::default();
        let base_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("base scan failed: {error}"));

        repo.write_file("src/inner.rs", b"pub fn helper() -> i32 { 2 }\n");
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));

        let changed_paths = BTreeSet::from([String::from("src/inner.rs")]);
        let merged = merge_delta_snapshot(&base_snapshot, &target_snapshot, &changed_paths);

        assert_eq!(merged.snapshot_id, target_snapshot.snapshot_id);
        assert!(
            merged
                .files
                .iter()
                .any(|file| file.relative_path == "src/lib.rs")
        );
        assert!(
            merged
                .files
                .iter()
                .any(|file| file.relative_path == "src/inner.rs")
        );
        let inner_count = merged
            .symbols
            .iter()
            .filter(|symbol| symbol.relative_path == "src/inner.rs")
            .count();
        assert!(inner_count > 0);
    }

    #[test]
    fn changed_paths_affect_verification_detects_manifest_paths() {
        let changed = BTreeSet::from([String::from("src/Cargo.toml")]);
        assert!(changed_paths_affect_verification(&changed));

        let changed = BTreeSet::from([String::from("src/lib.rs")]);
        assert!(!changed_paths_affect_verification(&changed));
    }

    #[test]
    fn normalize_changed_paths_rejects_outside_repo_root() {
        let repo = TempRepo::new("changed-path-outside-root");
        let outside_path = repo
            .root()
            .parent()
            .unwrap_or_else(|| panic!("temp repo should have a parent"))
            .join("outside.rs");
        let changed_paths = vec![outside_path.to_string_lossy().into_owned()];

        let result = normalize_changed_paths(repo.root(), &changed_paths);

        assert!(result.is_err());
        let error = match result {
            Ok(_) => panic!("normalize_changed_paths should fail for out-of-root paths"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("outside repo root"));
    }

    #[test]
    fn normalize_changed_paths_rejects_parent_traversal() {
        let repo = TempRepo::new("changed-path-parent-traversal");
        let changed_paths = vec!["../src/lib.rs".to_string()];

        let result = normalize_changed_paths(repo.root(), &changed_paths);

        assert!(result.is_err());
        let error = match result {
            Ok(_) => panic!("normalize_changed_paths should fail for parent traversal"),
            Err(error) => error.to_string(),
        };
        assert!(error.contains("parent traversal"));
    }

    #[test]
    fn resolve_changed_paths_expands_directory_prefix_inputs() {
        let repo = TempRepo::new("changed-path-directory-expansion");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"mod inner;\n");
        repo.write_file("src/inner.rs", b"pub fn helper() {}\n");
        repo.write_file("README.md", b"# demo\n");

        let scanner = RepositoryScanner::default();
        let base_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("base scan failed: {error}"));
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));
        let inputs = BTreeSet::from([String::from("src")]);

        let resolution = resolve_changed_paths_for_delta(&inputs, &base_snapshot, &target_snapshot);

        assert!(resolution.unresolved_inputs.is_empty());
        assert_eq!(resolution.directory_expansions.len(), 1);
        assert!(
            resolution
                .resolved_paths
                .iter()
                .any(|path| path == "src/lib.rs")
        );
        assert!(
            resolution
                .resolved_paths
                .iter()
                .any(|path| path == "src/inner.rs")
        );
        assert!(
            !resolution
                .resolved_paths
                .iter()
                .any(|path| path == "README.md")
        );
    }

    #[test]
    fn resolve_changed_paths_tracks_unresolved_entries() {
        let repo = TempRepo::new("changed-path-unresolved");
        repo.write_file(
            "Cargo.toml",
            b"[package]\nname = \"demo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        repo.write_file("src/lib.rs", b"pub fn stable() {}\n");

        let scanner = RepositoryScanner::default();
        let base_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("base".to_string()),
            })
            .unwrap_or_else(|error| panic!("base scan failed: {error}"));
        let target_snapshot = scanner
            .scan(&RepositoryTarget {
                root: repo.root().to_string_lossy().into_owned(),
                revision: Some("target".to_string()),
            })
            .unwrap_or_else(|error| panic!("target scan failed: {error}"));
        let inputs = BTreeSet::from([String::from("src/missing.rs")]);

        let resolution = resolve_changed_paths_for_delta(&inputs, &base_snapshot, &target_snapshot);

        assert!(resolution.resolved_paths.is_empty());
        assert_eq!(
            resolution.unresolved_inputs,
            vec!["src/missing.rs".to_string()]
        );
    }

    fn l2_test_policy() -> L2RelationalPolicy {
        L2RelationalPolicy {
            max_pairs: 16,
            max_candidate_functions: 64,
            timeout_ms: 2_000,
            alignment_mode: CliL2AlignmentMode::SignatureAware,
            min_alignment_score: 60,
        }
    }

    fn theorem_test_contract() -> TheoremContract {
        TheoremContract {
            contract_id: "demo-contract".to_string(),
            observable_outputs: vec!["stdout".to_string()],
            error_behavior: vec!["exit codes".to_string()],
            side_effects: vec!["none".to_string()],
            preconditions: vec!["same inputs".to_string()],
            environment_assumptions: vec!["stable filesystem".to_string()],
        }
    }

    fn theorem_test_policy() -> TheoremPolicy {
        TheoremPolicy {
            max_obligations: 16,
            max_candidate_functions: 64,
            timeout_ms: 2_000,
            flaky_retries: 1,
            stability_runs: 1,
            max_parallelism: 1,
            solver_mode: CliTheoremSolverMode::SmtZ3,
            solver_path: None,
            solver_timeout_ms: 10_000,
            solver_fallback_relational: true,
            replay_out: None,
            persist_replay: false,
            translation_validation: false,
            alive2_path: None,
            translation_max_obligations: 32,
            translation_timeout_ms: 1_000,
            translation_ir_mode: CliTranslationIrMode::RequireCompilerIr,
            translation_run_status_mode: CliTranslationRunStatusMode::Override,
        }
    }

    fn theorem_test_args(contract_file: Option<PathBuf>) -> WhatChangedSemanticallyArgs {
        WhatChangedSemanticallyArgs {
            r#ref: "HEAD".to_string(),
            stage: CliSemanticDiffStage::L0StructuralDelta,
            l1_kani_mode: CliL1KaniMode::Probe,
            l1_max_rust_files: 8,
            l1_max_functions: 16,
            l1_timeout_ms: 15_000,
            l2_alignment_mode: CliL2AlignmentMode::SignatureAware,
            l2_max_pairs: 24,
            l2_max_candidate_functions: 96,
            l2_timeout_ms: 15_000,
            l2_min_alignment_score: 65,
            theorem_contract_file: contract_file,
            theorem_max_obligations: 128,
            theorem_max_candidate_functions: 256,
            theorem_timeout_ms: 30_000,
            theorem_flaky_retries: 1,
            theorem_stability_runs: 1,
            theorem_max_parallelism: 1,
            theorem_solver_mode: CliTheoremSolverMode::SmtZ3,
            theorem_solver_path: None,
            theorem_solver_timeout_ms: 10_000,
            theorem_solver_fallback_relational: true,
            theorem_replay_out: None,
            theorem_persist_replay: true,
            theorem_translation_validation: true,
            theorem_alive2_path: None,
            theorem_translation_max_obligations: 32,
            theorem_translation_timeout_ms: 8_000,
            theorem_translation_ir_mode: CliTranslationIrMode::RequireCompilerIr,
            theorem_translation_run_status_mode: CliTranslationRunStatusMode::Override,
            repo_root: None,
            revision: None,
            serving: CliServingArgs {
                overlay_kind: CliOverlayKind::None,
                claim_scope: CliOverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
                snapshot_status: CliSnapshotStatus::Available,
                freshness_status: CliFreshnessStatus::Current,
                serving_health: CliServingHealth::Normal,
            },
        }
    }

    fn unique_suffix() -> u128 {
        match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_nanos(),
            Err(error) => error.duration().as_nanos(),
        }
    }
}
