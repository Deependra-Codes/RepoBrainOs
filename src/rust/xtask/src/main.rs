use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use repobrain_broker::{SnapshotBrokerInput, SnapshotContextBroker};
use repobrain_domain::{
    ConsumerType, ContextRequest, CoverageSlot, CoverageStatus, FreshnessRequirement, ModelClass,
    ModelProfile, OverlayClaimScope, OverlayKind, QueryClassification, RequestDepth,
    ScaffoldingLevel, TaskType,
};
use repobrain_graph::{GraphStore, SnapshotGraphStore};
use repobrain_ingest::{
    RepositoryInventorySnapshot, RepositoryScanner, RepositoryTarget, exact_path_lookup,
    exact_symbol_lookup,
};
use repobrain_serving::{FreshnessStatus, ReadinessAssessment, ServingHealth, SnapshotStatus};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(name = "repobrain-xtask")]
#[command(about = "Repo-native automation for RepoBrain OS")]
struct Cli {
    #[command(subcommand)]
    command: CommandKind,
}

#[derive(Debug, Subcommand)]
enum CommandKind {
    Doctor,
    Check,
    Quality,
    Perf,
    Eval,
    Policy,
    Sync,
    Fmt,
    InstallGitHooks,
    Quickstart,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = workspace_root()?;

    match cli.command {
        CommandKind::Doctor => doctor(&root),
        CommandKind::Check => check(&root),
        CommandKind::Quality => quality(&root),
        CommandKind::Perf => perf(&root),
        CommandKind::Eval => eval(&root),
        CommandKind::Policy => policy(&root),
        CommandKind::Sync => sync(&root),
        CommandKind::Fmt => fmt(&root),
        CommandKind::InstallGitHooks => install_git_hooks(&root),
        CommandKind::Quickstart => {
            quickstart();
            Ok(())
        }
    }
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .context("failed to resolve workspace root from xtask manifest dir")
}

fn doctor(root: &Path) -> Result<()> {
    println!("RepoBrain OS doctor");
    println!("workspace: {}", root.display());

    for command in ["cargo", "python", "node", "pnpm"] {
        ensure_command(command)?;
    }

    ensure_subcommand("cargo", &["fmt", "--version"])?;
    ensure_subcommand("cargo", &["clippy", "--version"])?;

    for path in [
        "README.md",
        "AGENTS.md",
        "CLAUDE.md",
        "docs/README.md",
        "docs/sdd-008-trust-readiness-and-verification-model.md",
        "docs/standards/ENGINEERING_QUALITY_BASELINE.md",
        "docs/standards/AGENT_GUARDRAIL_SYSTEM.md",
        "docs/standards/ENGINEERING_EXECUTION_POLICY.md",
        "docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md",
        "docs/standards/REPO_SYNC_POLICY.md",
        "docs/standards/TYPE_TDD_SPEC_AGENTIC_DISCIPLINE.md",
        ".github/copilot-instructions.md",
        "plans/v1-foundation-plan.md",
        "schemas/README.md",
        "biome.json",
    ] {
        let full = root.join(path);
        if !full.exists() {
            bail!("missing expected repo file: {}", full.display());
        }
    }

    println!("toolchain: ok");
    println!("repo shape: ok");
    println!("suggested next step: cargo xtask quality");

    Ok(())
}

fn check(root: &Path) -> Result<()> {
    format_check(root)?;
    quality(root)?;
    run(root, "cargo", &["check"], &[])?;
    run_in(
        &root.join("src/python/repobrain_research"),
        "python",
        &["-m", "unittest", "discover", "tests"],
        &[(
            "PYTHONPATH",
            root.join("src/python/repobrain_research/src")
                .to_string_lossy()
                .into_owned(),
        )],
    )?;

    println!("RepoBrain OS check: all green");

    Ok(())
}

fn quality(root: &Path) -> Result<()> {
    policy(root)?;
    sync(root)?;
    run(
        root,
        "cargo",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    run(
        root,
        "python",
        &["-m", "ruff", "check", "src/python/repobrain_research"],
        &[],
    )?;
    run(
        root,
        "python",
        &[
            "-m",
            "mypy",
            "--config-file",
            "src/python/repobrain_research/pyproject.toml",
            "src/python/repobrain_research/src",
            "src/python/repobrain_research/tests",
        ],
        &[],
    )?;
    run(root, "pnpm", &["quality:check"], &[])?;
    run(root, "pnpm", &["mcp:typecheck"], &[])?;

    println!("RepoBrain OS quality: all green");

    Ok(())
}

fn perf(_root: &Path) -> Result<()> {
    let fixture = PerfRepo::new("hot-path")?;
    write_perf_fixture(&fixture, PERF_FIXTURE_MODULE_COUNT)?;
    let snapshot = scan_perf_fixture(&fixture)?;
    let blast_radius = measure_blast_radius_perf(&snapshot)?;
    let get_brief = measure_get_brief_perf(&snapshot)?;

    println!("RepoBrain OS perf");
    println!("fixture modules: {PERF_FIXTURE_MODULE_COUNT}");
    println!(
        "blast-radius: p50={} p95={} mean={} max={}",
        render_duration(blast_radius.p50()),
        render_duration(blast_radius.p95()),
        render_duration(blast_radius.mean()),
        render_duration(blast_radius.max()),
    );
    println!(
        "get-brief: p50={} p95={} mean={} max={}",
        render_duration(get_brief.p50()),
        render_duration(get_brief.p95()),
        render_duration(get_brief.mean()),
        render_duration(get_brief.max()),
    );

    ensure_perf_threshold(
        "blast-radius p95",
        blast_radius.p95(),
        BLAST_RADIUS_P95_THRESHOLD,
    )?;
    ensure_perf_threshold("get-brief p95", get_brief.p95(), GET_BRIEF_P95_THRESHOLD)?;

    println!("RepoBrain OS perf: hot-path thresholds are within budget");

    Ok(())
}

fn eval(_root: &Path) -> Result<()> {
    let fixture = PerfRepo::new("retrieval-eval")?;
    write_eval_fixture(&fixture)?;
    let snapshot = scan_perf_fixture(&fixture)?;
    let broker = SnapshotContextBroker::default();
    let readiness_assessment = ready_assessment();
    let scenarios = eval_scenarios();
    let observations =
        run_eval_observations(&snapshot, &broker, &readiness_assessment, &scenarios)?;
    let summary = EvalSummary::from_observations(&observations)?;
    print_eval_summary(&summary, scenarios.len());
    ensure_eval_guardrails(&summary)?;

    println!("RepoBrain OS eval: retrieval quality and robustness guardrails are within budget");

    Ok(())
}

fn run_eval_observations(
    snapshot: &RepositoryInventorySnapshot,
    broker: &SnapshotContextBroker,
    readiness_assessment: &ReadinessAssessment,
    scenarios: &[EvalScenario],
) -> Result<Vec<EvalObservation>> {
    let mut observations = Vec::new();

    for scenario in scenarios {
        for mutation in EvalMutation::ALL {
            let request = scenario.request(mutation);
            let started = Instant::now();
            let pack = broker.get_brief(SnapshotBrokerInput {
                request: request.clone(),
                snapshot,
                readiness_assessment: readiness_assessment.clone(),
                overlay_kind: OverlayKind::None,
                claim_scope: OverlayClaimScope::SnapshotConfirmed,
                overlay_hash: None,
                touched_paths: Vec::new(),
            })?;
            let elapsed = started.elapsed();
            let baseline_required_slot_hit_count = anchor_only_baseline_hit_count(
                snapshot,
                request.scope_hint.as_deref(),
                &pack.coverage_audit.required_slots,
            );
            observations.push(EvalObservation::from_pack(
                scenario.name,
                mutation,
                scenario.expected_classification,
                elapsed,
                &pack,
                baseline_required_slot_hit_count,
            ));
        }
    }

    Ok(observations)
}

fn print_eval_summary(summary: &EvalSummary, scenario_count: usize) {
    println!("RepoBrain OS eval");
    println!(
        "scenario-count={scenario_count} mutation-count-per-scenario={}",
        EvalMutation::ALL.len()
    );
    println!("run-count={}", summary.run_count);
    println!(
        "get-brief-latency: p50={} p95={} mean={} max={}",
        render_duration(summary.latency.p50()),
        render_duration(summary.latency.p95()),
        render_duration(summary.latency.mean()),
        render_duration(summary.latency.max()),
    );
    println!(
        "classification-match-rate={} coverage-sufficient-rate={}",
        render_rate(summary.classification_match_rate),
        render_rate(summary.coverage_sufficient_rate),
    );
    println!(
        "required-slot-hit-rate={} anchor-only-baseline-hit-rate={} uplift-delta={}",
        render_rate(summary.required_slot_hit_rate),
        render_rate(summary.baseline_required_slot_hit_rate),
        render_rate(summary.required_slot_uplift_delta),
    );
    println!(
        "mutation-slot-retention={} mutation-evidence-overlap={}",
        render_rate(summary.mutation_slot_retention_mean),
        render_rate(summary.mutation_evidence_overlap_mean),
    );
    if let Some(efficiency) = summary.healed_slots_per_ms {
        println!(
            "second-pass-efficiency={efficiency:.4} healed-slot/ms (aggregate slot_gains / retrieval_cost_ms)"
        );
    } else {
        println!("second-pass-efficiency=n/a (no slot-cost samples)");
    }
    if summary.decision_cost_ms.is_empty() {
        println!("decision-evidence-slot-cost=n/a (no decision slot cost samples)");
    } else {
        println!(
            "decision-evidence-slot-cost-ms: p50={} p95={} mean={} max={}",
            render_float_ms(summary.decision_cost_ms.p50()),
            render_float_ms(summary.decision_cost_ms.p95()),
            render_float_ms(summary.decision_cost_ms.mean()),
            render_float_ms(summary.decision_cost_ms.max()),
        );
    }
}

fn ensure_eval_guardrails(summary: &EvalSummary) -> Result<()> {
    ensure_perf_threshold(
        "eval get-brief p95",
        summary.latency.p95(),
        EVAL_GET_BRIEF_P95_THRESHOLD,
    )?;
    ensure_rate_floor(
        "eval classification match rate",
        summary.classification_match_rate,
        EVAL_CLASSIFICATION_MATCH_FLOOR,
    )?;
    ensure_rate_floor(
        "eval coverage sufficient rate",
        summary.coverage_sufficient_rate,
        EVAL_COVERAGE_SUFFICIENCY_FLOOR,
    )?;
    ensure_rate_floor(
        "eval required slot hit rate",
        summary.required_slot_hit_rate,
        EVAL_REQUIRED_SLOT_HIT_FLOOR,
    )?;
    ensure_rate_floor(
        "eval required slot uplift delta",
        summary.required_slot_uplift_delta,
        EVAL_REQUIRED_SLOT_UPLIFT_FLOOR,
    )?;
    ensure_rate_floor(
        "eval mutation slot retention",
        summary.mutation_slot_retention_mean,
        EVAL_MUTATION_SLOT_RETENTION_FLOOR,
    )?;
    if !summary.decision_cost_ms.is_empty() {
        ensure_float_threshold(
            "eval decision-evidence slot retrieval_cost_ms p95",
            summary.decision_cost_ms.p95(),
            EVAL_DECISION_SLOT_COST_P95_MS_THRESHOLD,
        )?;
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvalMutation {
    Canonical,
    Terse,
    Typo,
    SymbolicNoise,
}

impl EvalMutation {
    const ALL: [Self; 4] = [
        Self::Canonical,
        Self::Terse,
        Self::Typo,
        Self::SymbolicNoise,
    ];
}

#[derive(Debug)]
struct EvalScenario {
    name: &'static str,
    task_type: TaskType,
    expected_classification: QueryClassification,
    goal: &'static str,
    question: &'static str,
    scope_hint: Option<&'static str>,
}

impl EvalScenario {
    fn request(&self, mutation: EvalMutation) -> ContextRequest {
        ContextRequest {
            goal: self.goal.to_string(),
            task_type: self.task_type,
            consumer_type: ConsumerType::XtaskPerf,
            model_profile: ModelProfile {
                id: "frontier-eval".to_string(),
                class: ModelClass::FrontierAgent,
                max_context_tokens: 128_000,
                preferred_scaffolding_level: ScaffoldingLevel::Minimal,
                notes: vec!["xtask eval harness".to_string()],
            },
            question: mutated_question(self.question, mutation),
            scope_hint: self.scope_hint.map(ToString::to_string),
            token_budget: 4_096,
            latency_budget: Some(900),
            depth: RequestDepth::Standard,
            freshness_requirement: Some(FreshnessRequirement::FreshPreferred),
            include_evidence: true,
        }
    }
}

#[derive(Debug)]
struct EvalObservation {
    scenario: String,
    mutation: EvalMutation,
    expected_classification: QueryClassification,
    observed_classification: QueryClassification,
    elapsed: Duration,
    coverage_sufficient: bool,
    required_slot_count: usize,
    satisfied_required_slot_count: usize,
    baseline_required_slot_hit_count: usize,
    required_slot_sufficient: BTreeMap<String, bool>,
    evidence_ids: BTreeSet<String>,
    slot_cost_ms: BTreeMap<String, f64>,
    slot_gains: BTreeMap<String, u32>,
}

impl EvalObservation {
    fn from_pack(
        scenario: &str,
        mutation: EvalMutation,
        expected_classification: QueryClassification,
        elapsed: Duration,
        pack: &repobrain_domain::BriefingPack,
        baseline_required_slot_hit_count: usize,
    ) -> Self {
        let required_slots = pack
            .coverage_audit
            .required_slots
            .iter()
            .map(|slot| coverage_slot_label(*slot).to_string())
            .collect::<BTreeSet<_>>();
        let mut required_slot_sufficient = BTreeMap::new();
        let mut slot_cost_ms = BTreeMap::new();
        let mut slot_gains = BTreeMap::new();

        for slot_audit in &pack.coverage_audit.slot_results {
            let slot_label = coverage_slot_label(slot_audit.slot).to_string();
            required_slot_sufficient.insert(
                slot_label.clone(),
                status_is_sufficient_for_eval(slot_audit.status),
            );
            if let Some(cost_ms) =
                parse_detail_float_metric(&slot_audit.detail, "retrieval_cost_ms=")
            {
                slot_cost_ms.insert(slot_label.clone(), cost_ms);
            }
            if let Some(gains) = parse_detail_u32_metric(&slot_audit.detail, "slot_gains=") {
                slot_gains.insert(slot_label, gains);
            }
        }
        let satisfied_required_slot_count = required_slots
            .iter()
            .filter(|slot_label| {
                required_slot_sufficient
                    .get((*slot_label).as_str())
                    .copied()
                    .unwrap_or(false)
            })
            .count();
        let evidence_ids = pack
            .evidence_index
            .iter()
            .map(|receipt| receipt.id.clone())
            .collect::<BTreeSet<_>>();

        Self {
            scenario: scenario.to_string(),
            mutation,
            expected_classification,
            observed_classification: pack.query_classification,
            elapsed,
            coverage_sufficient: pack.coverage_audit.sufficient,
            required_slot_count: required_slots.len(),
            satisfied_required_slot_count,
            baseline_required_slot_hit_count,
            required_slot_sufficient,
            evidence_ids,
            slot_cost_ms,
            slot_gains,
        }
    }

    fn classification_matches(&self) -> bool {
        self.expected_classification == self.observed_classification
    }

    fn total_slot_cost_ms(&self) -> f64 {
        self.slot_cost_ms.values().sum()
    }

    fn total_slot_gains(&self) -> u32 {
        self.slot_gains.values().sum()
    }
}

#[derive(Debug)]
struct EvalSummary {
    run_count: usize,
    latency: PerfMeasurement,
    classification_match_rate: f64,
    coverage_sufficient_rate: f64,
    required_slot_hit_rate: f64,
    baseline_required_slot_hit_rate: f64,
    required_slot_uplift_delta: f64,
    mutation_slot_retention_mean: f64,
    mutation_evidence_overlap_mean: f64,
    healed_slots_per_ms: Option<f64>,
    decision_cost_ms: FloatMeasurement,
}

impl EvalSummary {
    fn from_observations(observations: &[EvalObservation]) -> Result<Self> {
        if observations.is_empty() {
            bail!("eval harness produced no observations");
        }

        let latency = latency_from_observations(observations);
        let rates = eval_rate_totals(observations);
        let (mutation_slot_retention_mean, mutation_evidence_overlap_mean) =
            mutation_robustness_means(observations);
        let healed_slots_per_ms = healed_slots_per_ms(observations);
        let decision_cost_ms = decision_cost_distribution(observations);

        Ok(Self {
            run_count: observations.len(),
            latency,
            classification_match_rate: rates.classification_match_rate,
            coverage_sufficient_rate: rates.coverage_sufficient_rate,
            required_slot_hit_rate: rates.required_slot_hit_rate,
            baseline_required_slot_hit_rate: rates.baseline_required_slot_hit_rate,
            required_slot_uplift_delta: rates.required_slot_uplift_delta,
            mutation_slot_retention_mean,
            mutation_evidence_overlap_mean,
            healed_slots_per_ms,
            decision_cost_ms,
        })
    }
}

#[derive(Debug)]
struct EvalRateTotals {
    classification_match_rate: f64,
    coverage_sufficient_rate: f64,
    required_slot_hit_rate: f64,
    baseline_required_slot_hit_rate: f64,
    required_slot_uplift_delta: f64,
}

fn latency_from_observations(observations: &[EvalObservation]) -> PerfMeasurement {
    let mut latency_samples = observations
        .iter()
        .map(|observation| observation.elapsed.as_micros())
        .collect::<Vec<_>>();
    latency_samples.sort_unstable();

    PerfMeasurement {
        samples_micros: latency_samples,
    }
}

fn eval_rate_totals(observations: &[EvalObservation]) -> EvalRateTotals {
    let run_count = observations.len();
    let classification_match_count = observations
        .iter()
        .filter(|observation| observation.classification_matches())
        .count();
    let coverage_sufficient_count = observations
        .iter()
        .filter(|observation| observation.coverage_sufficient)
        .count();
    let required_slot_count = observations
        .iter()
        .map(|observation| observation.required_slot_count)
        .sum::<usize>();
    let required_slot_hit_count = observations
        .iter()
        .map(|observation| observation.satisfied_required_slot_count)
        .sum::<usize>();
    let baseline_required_slot_hit_count = observations
        .iter()
        .map(|observation| observation.baseline_required_slot_hit_count)
        .sum::<usize>();
    let classification_match_rate =
        usize_ratio(classification_match_count, run_count).unwrap_or(0.0);
    let coverage_sufficient_rate = usize_ratio(coverage_sufficient_count, run_count).unwrap_or(0.0);
    let required_slot_hit_rate =
        usize_ratio(required_slot_hit_count, required_slot_count).unwrap_or(0.0);
    let baseline_required_slot_hit_rate =
        usize_ratio(baseline_required_slot_hit_count, required_slot_count).unwrap_or(0.0);

    EvalRateTotals {
        classification_match_rate,
        coverage_sufficient_rate,
        required_slot_hit_rate,
        baseline_required_slot_hit_rate,
        required_slot_uplift_delta: (required_slot_hit_rate - baseline_required_slot_hit_rate)
            .max(0.0),
    }
}

fn mutation_robustness_means(observations: &[EvalObservation]) -> (f64, f64) {
    let mut observations_by_scenario = BTreeMap::<String, Vec<&EvalObservation>>::new();
    for observation in observations {
        observations_by_scenario
            .entry(observation.scenario.clone())
            .or_default()
            .push(observation);
    }

    let mut mutation_slot_retention_samples = Vec::new();
    let mut mutation_evidence_overlap_samples = Vec::new();
    for grouped_observations in observations_by_scenario.values() {
        let Some(canonical) = grouped_observations
            .iter()
            .copied()
            .find(|observation| observation.mutation == EvalMutation::Canonical)
        else {
            continue;
        };

        for observation in grouped_observations {
            if observation.mutation == EvalMutation::Canonical {
                continue;
            }
            mutation_slot_retention_samples.push(slot_retention(canonical, observation));
            mutation_evidence_overlap_samples.push(jaccard_index(
                &canonical.evidence_ids,
                &observation.evidence_ids,
            ));
        }
    }

    (
        mean(&mutation_slot_retention_samples).unwrap_or(1.0),
        mean(&mutation_evidence_overlap_samples).unwrap_or(1.0),
    )
}

fn healed_slots_per_ms(observations: &[EvalObservation]) -> Option<f64> {
    let total_slot_cost_ms = observations
        .iter()
        .map(EvalObservation::total_slot_cost_ms)
        .sum::<f64>();
    if total_slot_cost_ms <= 0.0 {
        return None;
    }

    let total_slot_gains = observations
        .iter()
        .map(EvalObservation::total_slot_gains)
        .sum::<u32>();
    Some(f64::from(total_slot_gains) / total_slot_cost_ms)
}

fn decision_cost_distribution(observations: &[EvalObservation]) -> FloatMeasurement {
    let mut decision_cost_ms = FloatMeasurement::default();
    for observation in observations {
        if let Some(cost_ms) = observation.slot_cost_ms.get("decision_evidence")
            && *cost_ms > 0.0
        {
            decision_cost_ms.push(*cost_ms);
        }
    }
    decision_cost_ms.sort();

    decision_cost_ms
}

#[derive(Debug, Default)]
struct FloatMeasurement {
    samples: Vec<f64>,
}

impl FloatMeasurement {
    fn push(&mut self, sample: f64) {
        if sample.is_finite() {
            self.samples.push(sample.max(0.0));
        }
    }

    fn sort(&mut self) {
        self.samples.sort_by(f64::total_cmp);
    }

    fn p50(&self) -> f64 {
        percentile_f64(&self.samples, 50, 100)
    }

    fn p95(&self) -> f64 {
        percentile_f64(&self.samples, 95, 100)
    }

    fn mean(&self) -> f64 {
        mean(&self.samples).unwrap_or(0.0)
    }

    fn max(&self) -> f64 {
        self.samples.last().copied().unwrap_or(0.0)
    }

    fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }
}

fn eval_scenarios() -> Vec<EvalScenario> {
    vec![
        EvalScenario {
            name: "entity_lookup",
            task_type: TaskType::Query,
            expected_classification: QueryClassification::EntityLookup,
            goal: "locate SnapshotLexicalIndexCache in the retrieval crate",
            question: "where is SnapshotLexicalIndexCache defined?",
            scope_hint: Some("SnapshotLexicalIndexCache"),
        },
        EvalScenario {
            name: "architecture_explanation",
            task_type: TaskType::ExplainFlow,
            expected_classification: QueryClassification::ArchitectureExplanation,
            goal: "explain retrieval flow for planner decisions",
            question: "how does retrieval frontier planning work end to end?",
            scope_hint: Some("src/planner.rs"),
        },
        EvalScenario {
            name: "safe_edit",
            task_type: TaskType::SafeEdit,
            expected_classification: QueryClassification::SafeEdit,
            goal: "prepare a safe edit for planner budget wiring",
            question: "what should I know before editing AdaptiveBudgetPlanner?",
            scope_hint: Some("AdaptiveBudgetPlanner"),
        },
        EvalScenario {
            name: "bug_fix",
            task_type: TaskType::SafeEdit,
            expected_classification: QueryClassification::BugFix,
            goal: "fix bug in retrieval frontier stop threshold handling",
            question: "failing threshold bug around RetrievalFrontierController",
            scope_hint: Some("RetrievalFrontierController"),
        },
        EvalScenario {
            name: "feature_implementation",
            task_type: TaskType::SafeEdit,
            expected_classification: QueryClassification::FeatureImplementation,
            goal: "add feature support for incremental lexical cache refresh",
            question: "implement feature support around snapshot pinning",
            scope_hint: Some("src/retrieval.rs"),
        },
        EvalScenario {
            name: "decision_why",
            task_type: TaskType::Query,
            expected_classification: QueryClassification::DecisionWhy,
            goal: "why did we split retrieval into lexical and bounded graph stages",
            question: "what rationale and tradeoff drove this decision?",
            scope_hint: Some("retrieval strategy"),
        },
    ]
}

fn write_eval_fixture(repo: &PerfRepo) -> Result<()> {
    repo.write_file(
        "Cargo.toml",
        "[package]\nname = \"repobrain-eval-fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;
    repo.write_file(
        "README.md",
        "# RepoBrain Eval Fixture\n\nThis fixture models retrieval, frontier control, and planner wiring for broker evaluation.\n",
    )?;
    repo.write_file(
        "src/lib.rs",
        "pub mod frontier;\npub mod planner;\npub mod retrieval;\npub mod verification;\n\npub use frontier::RetrievalFrontierController;\npub use planner::AdaptiveBudgetPlanner;\npub use retrieval::SnapshotLexicalIndexCache;\n",
    )?;
    repo.write_file(
        "src/retrieval.rs",
        "pub struct SnapshotLexicalIndexCache {\n    pub snapshot_pin_count: usize,\n}\n\nimpl SnapshotLexicalIndexCache {\n    pub fn new() -> Self {\n        Self { snapshot_pin_count: 0 }\n    }\n\n    pub fn incremental_refresh(&mut self, changed_files: usize) {\n        self.snapshot_pin_count = self.snapshot_pin_count.saturating_add(changed_files);\n    }\n}\n",
    )?;
    repo.write_file(
        "src/frontier.rs",
        "use crate::retrieval::SnapshotLexicalIndexCache;\n\npub struct RetrievalFrontierController {\n    floor: f64,\n}\n\nimpl RetrievalFrontierController {\n    pub fn new(floor: f64) -> Self {\n        Self { floor }\n    }\n\n    pub fn should_continue(&self, expected_healed_slots_per_ms: f64) -> bool {\n        expected_healed_slots_per_ms >= self.floor\n    }\n\n    pub fn forecast(&self, cache: &SnapshotLexicalIndexCache) -> f64 {\n        if cache.snapshot_pin_count == 0 {\n            self.floor\n        } else {\n            self.floor + 0.01\n        }\n    }\n}\n",
    )?;
    repo.write_file(
        "src/planner.rs",
        "use crate::frontier::RetrievalFrontierController;\nuse crate::retrieval::SnapshotLexicalIndexCache;\nuse crate::verification::verification_targets_for_planner;\n\npub struct AdaptiveBudgetPlanner {\n    frontier: RetrievalFrontierController,\n}\n\nimpl AdaptiveBudgetPlanner {\n    pub fn new() -> Self {\n        Self {\n            frontier: RetrievalFrontierController::new(0.01),\n        }\n    }\n\n    pub fn plan(&self, cache: &SnapshotLexicalIndexCache) -> usize {\n        let expected = self.frontier.forecast(cache);\n        let checks = verification_targets_for_planner();\n        if self.frontier.should_continue(expected) {\n            checks.len()\n        } else {\n            0\n        }\n    }\n}\n",
    )?;
    repo.write_file(
        "src/verification.rs",
        "pub fn verification_targets_for_planner() -> Vec<&'static str> {\n    vec![\"cargo test -p repobrain-broker\", \"cargo check -p repobrain-broker\"]\n}\n",
    )?;
    repo.write_file(
        "docs/adr/ADR-0001-retrieval-strategy.md",
        "# ADR-0001 Retrieval Strategy\n\nStatus: accepted\n\n## Decision\n\nUse lexical anchors first, then bounded graph expansion, then coverage audit.\n\n## Rationale\n\nThis decision keeps interactive latency low while preserving evidence-grounded retrieval.\n\n## Tradeoff\n\nA bounded frontier can miss long-range context, but this is preferable to unbounded noise in hot paths.\n",
    )?;
    repo.write_file(
        "docs/adr/ADR-0002-frontier-control.md",
        "# ADR-0002 Frontier Control\n\nStatus: accepted\n\n## Decision\n\nStop second-pass expansion when expected healed-slots per millisecond falls under class floor.\n\n## Rationale\n\nThis keeps retrieval budget proportional to practical slot healing gains.\n\n## Tradeoff\n\nConservative stopping can leave some retrievable slots unresolved on difficult scopes.\n",
    )?;

    Ok(())
}

fn mutated_question(question: &str, mutation: EvalMutation) -> String {
    match mutation {
        EvalMutation::Canonical => question.to_string(),
        EvalMutation::Terse => question
            .split_whitespace()
            .take(7)
            .collect::<Vec<_>>()
            .join(" "),
        EvalMutation::Typo => question
            .replace("retrieval", "retrival")
            .replace("decision", "decison")
            .replace("verification", "verfication"),
        EvalMutation::SymbolicNoise => format!("{question} (@_@)"),
    }
}

fn anchor_only_baseline_hit_count(
    snapshot: &RepositoryInventorySnapshot,
    scope_hint: Option<&str>,
    required_slots: &[CoverageSlot],
) -> usize {
    let anchor_hit = scope_hint.is_some_and(|scope| {
        exact_path_lookup(snapshot, scope).is_some()
            || !exact_symbol_lookup(snapshot, scope).is_empty()
    });

    required_slots
        .iter()
        .filter(|slot| matches!(slot, CoverageSlot::ExactAnchor) && anchor_hit)
        .count()
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

fn status_is_sufficient_for_eval(status: CoverageStatus) -> bool {
    matches!(
        status,
        CoverageStatus::Present | CoverageStatus::NotApplicable
    )
}

fn parse_detail_float_metric(detail: &str, metric_prefix: &str) -> Option<f64> {
    detail.split(';').find_map(|segment| {
        segment
            .trim()
            .strip_prefix(metric_prefix)
            .and_then(|value| value.parse::<f64>().ok())
    })
}

fn parse_detail_u32_metric(detail: &str, metric_prefix: &str) -> Option<u32> {
    detail.split(';').find_map(|segment| {
        segment
            .trim()
            .strip_prefix(metric_prefix)
            .and_then(|value| value.parse::<u32>().ok())
    })
}

fn slot_retention(canonical: &EvalObservation, mutated: &EvalObservation) -> f64 {
    let required_and_satisfied = canonical
        .required_slot_sufficient
        .iter()
        .filter(|(_, is_sufficient)| **is_sufficient)
        .map(|(slot, _)| slot.as_str())
        .collect::<BTreeSet<_>>();
    if required_and_satisfied.is_empty() {
        return 1.0;
    }
    let retained = required_and_satisfied
        .iter()
        .filter(|slot| {
            mutated
                .required_slot_sufficient
                .get(**slot)
                .copied()
                .unwrap_or(false)
        })
        .count();

    usize_ratio(retained, required_and_satisfied.len()).unwrap_or(0.0)
}

fn jaccard_index(left: &BTreeSet<String>, right: &BTreeSet<String>) -> f64 {
    if left.is_empty() && right.is_empty() {
        return 1.0;
    }
    let intersection = left.intersection(right).count();
    let union = left.union(right).count();
    usize_ratio(intersection, union).unwrap_or(0.0)
}

fn percentile_f64(samples: &[f64], numerator: usize, denominator: usize) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }

    let last_index = samples.len() - 1;
    let index = last_index.saturating_mul(numerator) / denominator;
    samples[index]
}

fn mean(samples: &[f64]) -> Option<f64> {
    if samples.is_empty() {
        return None;
    }
    let sum = samples.iter().sum::<f64>();

    Some(sum / usize_to_f64(samples.len()))
}

fn usize_ratio(numerator: usize, denominator: usize) -> Option<f64> {
    if denominator == 0 {
        return None;
    }

    Some(usize_to_f64(numerator) / usize_to_f64(denominator))
}

fn usize_to_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

fn ensure_rate_floor(label: &str, observed: f64, floor: f64) -> Result<()> {
    if observed + f64::EPSILON < floor {
        bail!("{label} dropped below floor: observed {observed:.4}, floor {floor:.4}");
    }

    Ok(())
}

fn ensure_float_threshold(label: &str, observed: f64, threshold: f64) -> Result<()> {
    if observed > threshold {
        bail!(
            "{label} exceeded threshold: observed {}, threshold {}",
            render_float_ms(observed),
            render_float_ms(threshold)
        );
    }

    Ok(())
}

fn render_rate(rate: f64) -> String {
    format!("{:.2}%", rate * 100.0)
}

fn render_float_ms(value_ms: f64) -> String {
    format!("{value_ms:.2} ms")
}

fn sync(root: &Path) -> Result<()> {
    ensure_index_sync(
        &root.join("docs/README.md"),
        &discover_markdown_files(&root.join("docs"), |name| {
            name.starts_with("sdd-") || name.starts_with("adr-")
        })?,
    )?;
    ensure_index_sync(
        &root.join("research/README.md"),
        &discover_markdown_files(&root.join("research"), |_| true)?,
    )?;
    ensure_index_sync(
        &root.join("docs/validation/README.md"),
        &discover_markdown_files(&root.join("docs/validation"), |_| true)?,
    )?;
    ensure_index_sync(
        &root.join("docs/standards/README.md"),
        &discover_markdown_files(&root.join("docs/standards"), |_| true)?,
    )?;
    ensure_index_sync(
        &root.join("docs/templates/README.md"),
        &discover_markdown_files(&root.join("docs/templates"), |_| true)?,
    )?;
    ensure_contract_sync(root)?;

    println!("RepoBrain OS sync: contracts and indexes are aligned");

    Ok(())
}

#[derive(Debug)]
struct PerfRepo {
    path: PathBuf,
}

impl PerfRepo {
    fn new(label: &str) -> Result<Self> {
        let mut path = env::temp_dir();
        path.push(format!(
            "repobrain-xtask-{label}-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&path)
            .with_context(|| format!("failed to create perf fixture {}", path.display()))?;

        Ok(Self { path })
    }

    fn root(&self) -> &Path {
        &self.path
    }

    fn write_file(&self, relative_path: &str, contents: &str) -> Result<()> {
        let path = self.path.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create parent {}", parent.display()))?;
        }

        fs::write(&path, contents)
            .with_context(|| format!("failed to write perf fixture {}", path.display()))
    }
}

impl Drop for PerfRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
struct PerfMeasurement {
    samples_micros: Vec<u128>,
}

impl PerfMeasurement {
    fn p50(&self) -> Duration {
        duration_from_micros(percentile(&self.samples_micros, 50, 100))
    }

    fn p95(&self) -> Duration {
        duration_from_micros(percentile(&self.samples_micros, 95, 100))
    }

    fn mean(&self) -> Duration {
        let total = self.samples_micros.iter().sum::<u128>();
        duration_from_micros(total / u128::try_from(self.samples_micros.len()).unwrap_or(1))
    }

    fn max(&self) -> Duration {
        duration_from_micros(*self.samples_micros.last().unwrap_or(&0))
    }
}

fn write_perf_fixture(repo: &PerfRepo, module_count: usize) -> Result<()> {
    repo.write_file(
        "Cargo.toml",
        "[package]\nname = \"repobrain-perf\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )?;

    let mut lib_rs = String::from("mod shared;\n");
    for index in 0..module_count {
        let _ = writeln!(lib_rs, "mod module_{index:03};");
    }
    lib_rs.push_str("pub use crate::shared::RepositoryScanner;\n");
    repo.write_file("src/lib.rs", &lib_rs)?;
    repo.write_file("src/shared.rs", "pub struct RepositoryScanner {}\n")?;

    for index in 0..module_count {
        repo.write_file(
            &format!("src/module_{index:03}.rs"),
            &format!(
                "use crate::shared::RepositoryScanner;\npub fn build_{index:03}() -> RepositoryScanner {{ RepositoryScanner {{}} }}\n"
            ),
        )?;
    }

    Ok(())
}

fn scan_perf_fixture(repo: &PerfRepo) -> Result<RepositoryInventorySnapshot> {
    RepositoryScanner::default()
        .scan(&RepositoryTarget {
            root: repo.root().to_string_lossy().into_owned(),
            revision: None,
        })
        .context("failed to scan perf fixture")
}

fn measure_blast_radius_perf(snapshot: &RepositoryInventorySnapshot) -> Result<PerfMeasurement> {
    measure_operation(PERF_WARMUP_ITERS, PERF_MEASURE_ITERS, || {
        let report = SnapshotGraphStore::new(snapshot).blast_radius("RepositoryScanner");
        if report.impacted_nodes.len() < 2 {
            bail!("blast-radius perf fixture produced too few impacted nodes");
        }
        let _ = std::hint::black_box(report.relationships.len() + report.impacted_nodes.len());
        Ok(())
    })
}

fn measure_get_brief_perf(snapshot: &RepositoryInventorySnapshot) -> Result<PerfMeasurement> {
    let broker = SnapshotContextBroker::default();
    let request = perf_request();
    let readiness_assessment = ready_assessment();

    measure_operation(PERF_WARMUP_ITERS, PERF_MEASURE_ITERS, || {
        let pack = broker.get_brief(SnapshotBrokerInput {
            request: request.clone(),
            snapshot,
            readiness_assessment: readiness_assessment.clone(),
            overlay_kind: OverlayKind::None,
            claim_scope: OverlayClaimScope::SnapshotConfirmed,
            overlay_hash: None,
            touched_paths: Vec::new(),
        })?;
        if pack.must_know.is_empty() {
            bail!("get-brief perf fixture produced an empty briefing pack");
        }
        let _ = std::hint::black_box(pack.evidence_index.len() + pack.suggested_files.len());
        Ok(())
    })
}

fn measure_operation(
    warmup_iters: usize,
    measure_iters: usize,
    mut operation: impl FnMut() -> Result<()>,
) -> Result<PerfMeasurement> {
    for _ in 0..warmup_iters {
        operation()?;
    }

    let mut samples_micros = Vec::with_capacity(measure_iters);
    for _ in 0..measure_iters {
        let start = Instant::now();
        operation()?;
        samples_micros.push(start.elapsed().as_micros());
    }
    samples_micros.sort_unstable();

    Ok(PerfMeasurement { samples_micros })
}

fn perf_request() -> ContextRequest {
    ContextRequest {
        goal: "measure graph and broker hot paths".to_string(),
        task_type: TaskType::SafeEdit,
        consumer_type: ConsumerType::XtaskPerf,
        model_profile: ModelProfile {
            id: "frontier-cli".to_string(),
            class: ModelClass::FrontierAgent,
            max_context_tokens: 128_000,
            preferred_scaffolding_level: ScaffoldingLevel::Minimal,
            notes: Vec::new(),
        },
        question: "what should I know before editing this?".to_string(),
        scope_hint: Some("RepositoryScanner".to_string()),
        token_budget: 4_096,
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

fn ensure_perf_threshold(label: &str, observed: Duration, threshold: Duration) -> Result<()> {
    if observed > threshold {
        bail!(
            "{label} exceeded threshold: observed {}, threshold {}",
            render_duration(observed),
            render_duration(threshold)
        );
    }

    Ok(())
}

fn percentile(samples: &[u128], numerator: usize, denominator: usize) -> u128 {
    if samples.is_empty() {
        return 0;
    }

    let last_index = samples.len() - 1;
    let index = last_index.saturating_mul(numerator) / denominator;
    samples[index]
}

fn duration_from_micros(micros: u128) -> Duration {
    Duration::from_micros(u64::try_from(micros).unwrap_or(u64::MAX))
}

fn render_duration(duration: Duration) -> String {
    format!("{:.3} ms", duration.as_secs_f64() * 1000.0)
}

fn policy(root: &Path) -> Result<()> {
    for path in [
        "AGENTS.md",
        "CLAUDE.md",
        ".github/copilot-instructions.md",
        "docs/standards/AGENT_GUARDRAIL_SYSTEM.md",
        "docs/standards/ENGINEERING_QUALITY_BASELINE.md",
        "docs/standards/ENGINEERING_EXECUTION_POLICY.md",
        "docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md",
        "docs/standards/REPO_SYNC_POLICY.md",
        "docs/standards/TYPE_TDD_SPEC_AGENTIC_DISCIPLINE.md",
        "docs/sdd-008-trust-readiness-and-verification-model.md",
        "docs/templates/INTENT_TEMPLATE.md",
        "docs/templates/RESEARCH_TEMPLATE.md",
        "docs/templates/SPEC_TEMPLATE.md",
        "docs/templates/VIBE_CODING_PROMPT_TEMPLATE.md",
        "docs/templates/VERIFICATION_RECORD_TEMPLATE.md",
        ".githooks/pre-commit",
    ] {
        let full = root.join(path);
        if !full.exists() {
            bail!("missing required policy artifact: {}", full.display());
        }
    }

    ensure_markdown_sections(
        &root.join("AGENTS.md"),
        &[
            "## Mission",
            "## Default Working Loop",
            "## Research-First Guardrail",
            "## Less-Code Guardrail",
            "## Data Structure And Complexity Guardrail",
            "## Verification Guardrail",
            "## Repo Commands",
        ],
    )?;
    ensure_max_lines(&root.join("AGENTS.md"), 260)?;

    ensure_markdown_sections(&root.join("CLAUDE.md"), &["## Claude Code"])?;
    ensure_max_lines(&root.join("CLAUDE.md"), 40)?;

    ensure_markdown_sections(
        &root.join("docs/sdd-008-trust-readiness-and-verification-model.md"),
        &[
            "## One-Line Decision",
            "## Trust Boundary",
            "## Readiness State Model",
            "## Snapshot And Overlay Model",
            "## Verification Planner",
            "## Decision",
        ],
    )?;
    ensure_performance_complexity_policy(root)?;
    ensure_graph_index_guardrails(root)?;
    ensure_ingest_lookup_guardrails(root)?;
    ensure_semantic_pipeline_guardrails(root)?;

    ensure_markdown_sections(
        &root.join(".github/copilot-instructions.md"),
        &[
            "## RepoBrain Defaults",
            "## Before Writing Code",
            "## Code Shape",
            "## Verification",
        ],
    )?;
    ensure_max_lines(&root.join(".github/copilot-instructions.md"), 140)?;

    println!("RepoBrain OS policy: doctrine artifacts and sync guardrails are present");

    Ok(())
}

fn ensure_performance_complexity_policy(root: &Path) -> Result<()> {
    ensure_performance_standard_sections(root)?;
    ensure_performance_templates(root)?;
    ensure_performance_supporting_docs(root)?;
    ensure_performance_sections_in_repo_artifacts(root)?;

    Ok(())
}

fn ensure_semantic_pipeline_guardrails(root: &Path) -> Result<()> {
    let root_module = root.join("src/rust/crates/repobrain-cli/src/semantic_pipeline.rs");
    ensure_max_lines(&root_module, 1_500)?;

    let module_dir = root.join("src/rust/crates/repobrain-cli/src/semantic_pipeline");
    for file in ["l1.rs", "l2.rs", "theorem.rs", "ivl.rs"] {
        let full = module_dir.join(file);
        if !full.exists() {
            bail!(
                "missing semantic pipeline module {} (module decomposition guardrail)",
                full.display()
            );
        }
    }

    ensure_max_lines(&module_dir.join("l1.rs"), 1_000)?;
    ensure_max_lines(&module_dir.join("l2.rs"), 1_500)?;
    ensure_max_lines(&module_dir.join("theorem.rs"), 2_800)?;
    ensure_max_lines(&module_dir.join("ivl.rs"), 1_600)?;

    Ok(())
}

fn ensure_performance_standard_sections(root: &Path) -> Result<()> {
    ensure_markdown_sections(
        &root.join("docs/standards/PERFORMANCE_AND_COMPLEXITY_DISCIPLINE.md"),
        &[
            "## One-Line Policy",
            "## Core Rules",
            "## Expected Artifacts For Scale-Sensitive Work",
            "## Review Questions",
            "## Decision",
            "Time, space, and data-structure quality are release criteria for scale-sensitive work.",
            "operation-cost table or equivalent cost summary",
        ],
    )
}

fn ensure_performance_templates(root: &Path) -> Result<()> {
    ensure_markdown_sections(
        &root.join("docs/templates/INTENT_TEMPLATE.md"),
        &[
            "## Workload And Performance Shape",
            "- Dominant operations and expected frequency:",
            "- Likely failure mode at 10x scale:",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("docs/templates/RESEARCH_TEMPLATE.md"),
        &[
            "## Engineering Impact",
            "- Workload assumptions:",
            "- Dominant operations and cost centers:",
            "- Candidate data structures / indexes:",
            "- Main rejected alternative and why:",
            "- Measurement plan or reason no benchmark is needed:",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("docs/templates/SPEC_TEMPLATE.md"),
        &[
            "## Workload And Complexity Notes",
            "- Dominant operations and expected frequency:",
            "- Main alternative considered:",
            "- Index ownership / maintenance notes:",
            "- Benchmark expectation or reason none is needed:",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("docs/templates/VERIFICATION_RECORD_TEMPLATE.md"),
        &[
            "## Performance / Complexity Validation",
            "- Workload exercised:",
            "- Measured:",
            "- Inferred:",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("docs/templates/VIBE_CODING_PROMPT_TEMPLATE.md"),
        &[
            "Treat time, space, and data-structure quality as release criteria for scale-sensitive work.",
            "Write the dominant operations, chosen structures, and memory tradeoffs before changing a hot or repo-scale path.",
            "If you cannot justify the structure with workload reasoning, stop and research before coding.",
            "Classify the path as hot, warm, or cold before adding complexity for performance.",
            "If you choose a simpler slower design, say why that slowness is acceptable for the real workload.",
        ],
    )
}

fn ensure_performance_supporting_docs(root: &Path) -> Result<()> {
    ensure_markdown_sections(
        &root.join("docs/standards/TYPE_TDD_SPEC_AGENTIC_DISCIPLINE.md"),
        &["### 4. Scale-sensitive specs need workload and complexity notes"],
    )?;
    ensure_markdown_sections(
        &root.join("docs/standards/ENGINEERING_QUALITY_BASELINE.md"),
        &[
            "[Performance And Complexity Discipline]",
            "performance-sensitive choices state workload, hot/cold path, dominant operations, data-structure choice, memory / allocation story, and measured-vs-inferred tradeoffs",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("docs/standards/AGENT_GUARDRAIL_SYSTEM.md"),
        &[
            "hot-path vs cold-path classification",
            "measured-vs-inferred performance reasoning when scale matters",
            "Time, space, and data-structure quality are release criteria for scale-sensitive work.",
        ],
    )?;
    ensure_markdown_sections(
        &root.join("AGENTS.md"),
        &[
            "classify the path as hot, warm, or cold before arguing for extra complexity",
            "if a simpler slower path is acceptable, say why that slowness is acceptable for this workload",
            "time, space, and data-structure quality are release criteria for scale-sensitive work",
            "if you cannot justify the structure with workload reasoning, stop and research before coding",
        ],
    )?;
    ensure_markdown_sections(
        &root.join(".github/copilot-instructions.md"),
        &[
            "treat time, space, and data-structure quality as release criteria for scale-sensitive work",
            "if a path is hot or repo-scale, state dominant operations, key costs, and memory tradeoffs",
        ],
    )
}

fn ensure_performance_sections_in_repo_artifacts(root: &Path) -> Result<()> {
    ensure_markdown_sections_for_discovered_files(
        &root.join("plans"),
        &discover_markdown_files(&root.join("plans"), |name| name.ends_with("-intent.md"))?,
        &["## Workload And Performance Shape"],
    )?;
    ensure_markdown_sections_for_discovered_files(
        &root.join("plans"),
        &discover_markdown_files(&root.join("plans"), |name| name.ends_with("-spec.md"))?,
        &["## Workload And Complexity Notes"],
    )?;
    ensure_markdown_sections_for_discovered_files(
        &root.join("docs/validation"),
        &discover_markdown_files(&root.join("docs/validation"), |name| {
            name.starts_with("VR-")
        })?,
        &[
            "## Performance / Complexity Validation",
            "- Measured:",
            "- Inferred:",
        ],
    )
}

fn ensure_graph_index_guardrails(root: &Path) -> Result<()> {
    let graph_path = root.join("src/rust/crates/repobrain-graph/src/lib.rs");
    let content = fs::read_to_string(&graph_path)
        .with_context(|| format!("failed to read graph source {}", graph_path.display()))?;

    if content.contains("self.snapshot.imports") || content.contains("&snapshot.imports") {
        bail!(
            "graph source {} directly accesses raw snapshot imports; use maintained lookups such as `direct_import_lookup` / `reverse_import_lookup` instead",
            graph_path.display()
        );
    }

    if content.contains("self.snapshot.symbols.iter()") {
        bail!(
            "graph source {} directly scans `self.snapshot.symbols`; use maintained lookups such as `exact_symbol_lookup` or `symbols_for_path` instead",
            graph_path.display()
        );
    }

    Ok(())
}

fn ensure_ingest_lookup_guardrails(root: &Path) -> Result<()> {
    let ingest_path = root.join("src/rust/crates/repobrain-ingest/src/lib.rs");
    let content = fs::read_to_string(&ingest_path)
        .with_context(|| format!("failed to read ingest source {}", ingest_path.display()))?;

    if content
        .contains(".symbols\n        .iter()\n        .find(|symbol| symbol.fact_id == fact_id)")
        || content.contains(".symbols.iter().find(|symbol| symbol.fact_id == fact_id)")
    {
        bail!(
            "ingest source {} performs a linear fact-id lookup over `symbols`; maintain an indexed fact-id lookup path instead",
            ingest_path.display()
        );
    }

    Ok(())
}

fn fmt(root: &Path) -> Result<()> {
    run(root, "cargo", &["fmt", "--all"], &[])?;
    run(
        root,
        "python",
        &["-m", "ruff", "format", "src/python/repobrain_research"],
        &[],
    )?;
    run(root, "pnpm", &["quality:format"], &[])?;

    println!("RepoBrain OS format: all green");

    Ok(())
}

fn install_git_hooks(root: &Path) -> Result<()> {
    ensure_command("git")?;
    run(root, "git", &["config", "core.hooksPath", ".githooks"], &[])?;

    println!("Git hooks configured to use .githooks/");

    Ok(())
}

fn quickstart() {
    println!("Read in this order:");
    println!("1. README.md");
    println!("2. docs/README.md");
    println!("3. plans/v1-foundation-plan.md");
    println!();
    println!("Primary commands:");
    println!("cargo xtask doctor");
    println!("cargo xtask fmt");
    println!("cargo xtask policy");
    println!("cargo xtask sync");
    println!("cargo xtask quality");
    println!("cargo xtask check");
    println!("cargo xtask perf");
    println!("cargo xtask eval");
}

fn unique_suffix() -> u128 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => error.duration().as_nanos(),
    }
}

fn ensure_command(command: &str) -> Result<()> {
    let status = Command::new(resolve_program(command))
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to execute `{command} --version`"))?;

    if !status.success() {
        bail!("command `{command}` is installed but not healthy");
    }

    Ok(())
}

fn ensure_subcommand(program: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(resolve_program(program))
        .args(args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("failed to execute `{program} {}`", args.join(" ")))?;

    if !status.success() {
        bail!(
            "command `{program} {}` is installed but not healthy",
            args.join(" ")
        );
    }

    Ok(())
}

fn format_check(root: &Path) -> Result<()> {
    run(root, "cargo", &["fmt", "--all", "--", "--check"], &[])?;
    run(
        root,
        "python",
        &[
            "-m",
            "ruff",
            "format",
            "--check",
            "src/python/repobrain_research",
        ],
        &[],
    )?;

    Ok(())
}

fn ensure_markdown_sections(path: &Path, sections: &[&str]) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read required policy file: {}", path.display()))?;

    for section in sections {
        if !content.contains(section) {
            bail!(
                "required policy file {} is missing section `{section}`",
                path.display()
            );
        }
    }

    Ok(())
}

fn ensure_max_lines(path: &Path, max_lines: usize) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read required policy file: {}", path.display()))?;
    let line_count = content.lines().count();

    if line_count > max_lines {
        bail!(
            "required policy file {} exceeds {max_lines} lines ({line_count})",
            path.display()
        );
    }

    Ok(())
}

fn ensure_markdown_sections_for_discovered_files(
    dir: &Path,
    files: &[String],
    sections: &[&str],
) -> Result<()> {
    for file in files {
        ensure_markdown_sections(&dir.join(file), sections)?;
    }

    Ok(())
}

fn discover_markdown_files(dir: &Path, filter: impl Fn(&str) -> bool) -> Result<Vec<String>> {
    let mut names = Vec::new();

    for entry in
        fs::read_dir(dir).with_context(|| format!("failed to read directory {}", dir.display()))?
    {
        let entry = entry.with_context(|| format!("failed to read entry in {}", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|file_name| file_name.to_str()) else {
            continue;
        };

        if name == "README.md"
            || !Path::new(name)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            || !filter(name)
        {
            continue;
        }

        names.push(name.to_string());
    }

    names.sort();
    Ok(names)
}

fn ensure_index_sync(index_path: &Path, expected_files: &[String]) -> Result<()> {
    let content = fs::read_to_string(index_path)
        .with_context(|| format!("failed to read index file {}", index_path.display()))?;

    for expected_file in expected_files {
        if !content.contains(expected_file) {
            bail!(
                "index file {} is missing entry for {}",
                index_path.display(),
                expected_file
            );
        }
    }

    Ok(())
}

fn ensure_contract_sync(root: &Path) -> Result<()> {
    let sources = ContractSources::load(root)?;

    ensure_model_profile_sync(root, &sources)?;
    ensure_context_request_sync(root, &sources)?;
    ensure_evidence_receipt_sync(root, &sources)?;
    ensure_impact_summary_sync(root, &sources)?;
    ensure_snapshot_binding_sync(root, &sources)?;
    ensure_overlay_scope_sync(root, &sources)?;
    ensure_verification_plan_sync(root, &sources)?;
    ensure_briefing_pack_sync(root, &sources)?;
    ensure_model_profile_enums_sync(root, &sources)?;
    ensure_request_enums_sync(root, &sources)?;
    ensure_overlay_scope_enums_sync(root, &sources)?;
    ensure_briefing_pack_enums_sync(root, &sources)?;
    ensure_impact_summary_enums_sync(root, &sources)?;

    Ok(())
}

struct ContractSources {
    rust: String,
    ts: String,
    python: String,
}

impl ContractSources {
    fn load(root: &Path) -> Result<Self> {
        Ok(Self {
            rust: fs::read_to_string(root.join("src/rust/crates/repobrain-domain/src/lib.rs"))
                .context("failed to read Rust contract surface")?,
            ts: fs::read_to_string(root.join("src/ts/packages/mcp-server/src/contracts.ts"))
                .context("failed to read TypeScript contract surface")?,
            python: fs::read_to_string(
                root.join("src/python/repobrain_research/src/repobrain_research/contracts.py"),
            )
            .context("failed to read Python contract surface")?,
        })
    }
}

fn ensure_model_profile_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/model-profile.schema.json",
        &[
            FieldMapping::new("id", "pub id: String,", "id: string;", "id: str"),
            FieldMapping::new(
                "class",
                "pub class: ModelClass,",
                "class: ModelClass;",
                "model_class: ModelClass",
            ),
            FieldMapping::new(
                "max_context_tokens",
                "pub max_context_tokens: u32,",
                "maxContextTokens: number;",
                "max_context_tokens: int",
            ),
            FieldMapping::new(
                "preferred_scaffolding_level",
                "pub preferred_scaffolding_level: ScaffoldingLevel,",
                "preferredScaffoldingLevel: ScaffoldingLevel;",
                "preferred_scaffolding_level: ScaffoldingLevel",
            ),
            FieldMapping::new(
                "notes",
                "pub notes: Vec<String>,",
                "notes?: string[];",
                "notes: list[str] = field(default_factory=list)",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_context_request_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/context-request.schema.json",
        &[
            FieldMapping::new("goal", "pub goal: String,", "goal: string;", "goal: str"),
            FieldMapping::new(
                "task_type",
                "pub task_type: TaskType,",
                "taskType: TaskType;",
                "task_type: TaskType",
            ),
            FieldMapping::new(
                "consumer_type",
                "pub consumer_type: ConsumerType,",
                "consumerType: ConsumerType;",
                "consumer_type: ConsumerType",
            ),
            FieldMapping::new(
                "model_profile",
                "pub model_profile: ModelProfile,",
                "modelProfile: ModelProfile;",
                "model_profile: ModelProfile",
            ),
            FieldMapping::new(
                "question",
                "pub question: String,",
                "question: string;",
                "question: str",
            ),
            FieldMapping::new(
                "scope_hint",
                "pub scope_hint: Option<String>,",
                "scopeHint?: string;",
                "scope_hint: str | None = None",
            ),
            FieldMapping::new(
                "token_budget",
                "pub token_budget: u32,",
                "tokenBudget: number;",
                "token_budget: int",
            ),
            FieldMapping::new(
                "latency_budget",
                "pub latency_budget: Option<u32>,",
                "latencyBudget?: number;",
                "latency_budget: int | None = None",
            ),
            FieldMapping::new(
                "depth",
                "pub depth: RequestDepth,",
                "depth: RequestDepth;",
                "depth: RequestDepth",
            ),
            FieldMapping::new(
                "freshness_requirement",
                "pub freshness_requirement: Option<FreshnessRequirement>,",
                "freshnessRequirement?: FreshnessRequirement;",
                "freshness_requirement: FreshnessRequirement | None = None",
            ),
            FieldMapping::new(
                "include_evidence",
                "pub include_evidence: bool,",
                "includeEvidence: boolean;",
                "include_evidence: bool",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_evidence_receipt_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/evidence-receipt.schema.json",
        &[
            FieldMapping::new("id", "pub id: String,", "id: string;", "id: str"),
            FieldMapping::new(
                "source_type",
                "pub source_type: String,",
                "sourceType: string;",
                "source_type: str",
            ),
            FieldMapping::new(
                "source_ref",
                "pub source_ref: String,",
                "sourceRef: string;",
                "source_ref: str",
            ),
            FieldMapping::new(
                "locator",
                "pub locator: String,",
                "locator: string;",
                "locator: str",
            ),
            FieldMapping::new(
                "snippet_hash",
                "pub snippet_hash: Option<String>,",
                "snippetHash?: string;",
                "snippet_hash: str | None = None",
            ),
            FieldMapping::new(
                "captured_at",
                "pub captured_at: String,",
                "capturedAt: string;",
                "captured_at: str",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_impact_summary_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/impact-summary.schema.json",
        &[
            FieldMapping::new(
                "reference",
                "pub reference: String,",
                "reference: string;",
                "reference: str",
            ),
            FieldMapping::new(
                "summary",
                "pub summary: String,",
                "summary: string;",
                "summary: str",
            ),
            FieldMapping::new(
                "changed_scope",
                "pub changed_scope: Vec<String>,",
                "changedScope: string[];",
                "changed_scope: list[str]",
            ),
            FieldMapping::new(
                "stale_concepts",
                "pub stale_concepts: Vec<String>,",
                "staleConcepts: string[];",
                "stale_concepts: list[str]",
            ),
            FieldMapping::new(
                "freshness_impact",
                "pub freshness_impact: FreshnessImpact,",
                "freshnessImpact: FreshnessImpact;",
                "freshness_impact: FreshnessImpact",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_snapshot_binding_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/snapshot-binding.schema.json",
        &[
            FieldMapping::new(
                "snapshot_id",
                "pub snapshot_id: String,",
                "snapshotId: string;",
                "snapshot_id: str",
            ),
            FieldMapping::new(
                "repo_root",
                "pub repo_root: String,",
                "repoRoot: string;",
                "repo_root: str",
            ),
            FieldMapping::new(
                "revision",
                "pub revision: Option<String>,",
                "revision?: string;",
                "revision: str | None = None",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_overlay_scope_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/overlay-scope.schema.json",
        &[
            FieldMapping::new(
                "kind",
                "pub kind: OverlayKind,",
                "kind: OverlayKind;",
                "kind: OverlayKind",
            ),
            FieldMapping::new(
                "claim_scope",
                "pub claim_scope: OverlayClaimScope,",
                "claimScope: OverlayClaimScope;",
                "claim_scope: OverlayClaimScope",
            ),
            FieldMapping::new(
                "overlay_hash",
                "pub overlay_hash: Option<String>,",
                "overlayHash?: string;",
                "overlay_hash: str | None = None",
            ),
            FieldMapping::new(
                "touched_paths",
                "pub touched_paths: Vec<String>,",
                "touchedPaths: string[];",
                "touched_paths: list[str]",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_verification_plan_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/verification-plan.schema.json",
        &[
            FieldMapping::new(
                "required_checks",
                "pub required_checks: Vec<String>,",
                "requiredChecks: string[];",
                "required_checks: list[str]",
            ),
            FieldMapping::new(
                "recommended_checks",
                "pub recommended_checks: Vec<String>,",
                "recommendedChecks: string[];",
                "recommended_checks: list[str]",
            ),
            FieldMapping::new(
                "invariants",
                "pub invariants: Vec<String>,",
                "invariants: string[];",
                "invariants: list[str]",
            ),
            FieldMapping::new(
                "coverage_gaps",
                "pub coverage_gaps: Vec<String>,",
                "coverageGaps: string[];",
                "coverage_gaps: list[str]",
            ),
            FieldMapping::new(
                "stop_conditions",
                "pub stop_conditions: Vec<String>,",
                "stopConditions: string[];",
                "stop_conditions: list[str]",
            ),
        ],
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

const BRIEFING_PACK_FIELD_MAPPINGS: &[FieldMapping] = &[
    FieldMapping::new(
        "task_type",
        "pub task_type: TaskType,",
        "taskType: TaskType;",
        "task_type: TaskType",
    ),
    FieldMapping::new(
        "query_classification",
        "pub query_classification: QueryClassification,",
        "queryClassification: QueryClassification;",
        "query_classification: QueryClassification",
    ),
    FieldMapping::new(
        "consumer_type",
        "pub consumer_type: ConsumerType,",
        "consumerType: ConsumerType;",
        "consumer_type: ConsumerType",
    ),
    FieldMapping::new(
        "model_profile",
        "pub model_profile: ModelProfile,",
        "modelProfile: ModelProfile;",
        "model_profile: ModelProfile",
    ),
    FieldMapping::new(
        "readiness_state",
        "pub readiness_state: ReadinessState,",
        "readinessState: ReadinessState;",
        "readiness_state: ReadinessState",
    ),
    FieldMapping::new(
        "snapshot_binding",
        "pub snapshot_binding: SnapshotBinding,",
        "snapshotBinding: SnapshotBinding;",
        "snapshot_binding: SnapshotBinding",
    ),
    FieldMapping::new(
        "overlay_scope",
        "pub overlay_scope: OverlayScope,",
        "overlayScope: OverlayScope;",
        "overlay_scope: OverlayScope",
    ),
    FieldMapping::new(
        "scaffolding_level",
        "pub scaffolding_level: ScaffoldingLevel,",
        "scaffoldingLevel: ScaffoldingLevel;",
        "scaffolding_level: ScaffoldingLevel",
    ),
    FieldMapping::new(
        "must_know",
        "pub must_know: Vec<BriefingItem>,",
        "mustKnow: BriefingItem[];",
        "must_know: list[BriefingItem]",
    ),
    FieldMapping::new(
        "relevant_flows",
        "pub relevant_flows: Vec<String>,",
        "relevantFlows: string[];",
        "relevant_flows: list[str]",
    ),
    FieldMapping::new(
        "relevant_decisions",
        "pub relevant_decisions: Vec<String>,",
        "relevantDecisions: string[];",
        "relevant_decisions: list[str]",
    ),
    FieldMapping::new(
        "fragile_zones",
        "pub fragile_zones: Vec<String>,",
        "fragileZones: string[];",
        "fragile_zones: list[str]",
    ),
    FieldMapping::new(
        "do_not_break",
        "pub do_not_break: Vec<String>,",
        "doNotBreak: string[];",
        "do_not_break: list[str]",
    ),
    FieldMapping::new(
        "suggested_files",
        "pub suggested_files: Vec<String>,",
        "suggestedFiles: string[];",
        "suggested_files: list[str]",
    ),
    FieldMapping::new(
        "reasoning_scaffold",
        "pub reasoning_scaffold: Vec<String>,",
        "reasoningScaffold: string[];",
        "reasoning_scaffold: list[str]",
    ),
    FieldMapping::new(
        "impact_summary",
        "pub impact_summary: Option<ImpactSummary>,",
        "impactSummary?: ImpactSummary;",
        "impact_summary: ImpactSummary | None",
    ),
    FieldMapping::new(
        "verification_plan",
        "pub verification_plan: VerificationPlan,",
        "verificationPlan: VerificationPlan;",
        "verification_plan: VerificationPlan",
    ),
    FieldMapping::new(
        "coverage_audit",
        "pub coverage_audit: CoverageAudit,",
        "coverageAudit: CoverageAudit;",
        "coverage_audit: CoverageAudit",
    ),
    FieldMapping::new(
        "verification_targets",
        "pub verification_targets: Vec<String>,",
        "verificationTargets: string[];",
        "verification_targets: list[str]",
    ),
    FieldMapping::new(
        "evidence_index",
        "pub evidence_index: Vec<EvidenceReceipt>,",
        "evidenceIndex: EvidenceReceipt[];",
        "evidence_index: list[EvidenceReceipt]",
    ),
];

const PERF_FIXTURE_MODULE_COUNT: usize = 192;
const PERF_WARMUP_ITERS: usize = 5;
const PERF_MEASURE_ITERS: usize = 40;
const BLAST_RADIUS_P95_THRESHOLD: Duration = Duration::from_millis(20);
const GET_BRIEF_P95_THRESHOLD: Duration = Duration::from_millis(40);
const EVAL_GET_BRIEF_P95_THRESHOLD: Duration = Duration::from_millis(55);
const EVAL_CLASSIFICATION_MATCH_FLOOR: f64 = 0.95;
const EVAL_COVERAGE_SUFFICIENCY_FLOOR: f64 = 0.60;
const EVAL_REQUIRED_SLOT_HIT_FLOOR: f64 = 0.78;
const EVAL_REQUIRED_SLOT_UPLIFT_FLOOR: f64 = 0.20;
const EVAL_MUTATION_SLOT_RETENTION_FLOOR: f64 = 0.70;
const EVAL_DECISION_SLOT_COST_P95_MS_THRESHOLD: f64 = 25.0;

fn ensure_briefing_pack_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_contract(
        root,
        "schemas/briefing-pack.schema.json",
        BRIEFING_PACK_FIELD_MAPPINGS,
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_model_profile_enums_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_enum(
        root,
        "schemas/model-profile.schema.json",
        "/properties/class/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/model-profile.schema.json",
        "/properties/preferred_scaffolding_level/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_request_enums_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_enum(
        root,
        "schemas/context-request.schema.json",
        "/properties/task_type/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/context-request.schema.json",
        "/properties/depth/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/context-request.schema.json",
        "/properties/freshness_requirement/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_overlay_scope_enums_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_enum(
        root,
        "schemas/overlay-scope.schema.json",
        "/properties/kind/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/overlay-scope.schema.json",
        "/properties/claim_scope/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_briefing_pack_enums_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/query_classification/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/scaffolding_level/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/readiness_state/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/must_know/items/properties/freshness/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/coverage_audit/properties/required_slots/items/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )?;
    ensure_schema_enum(
        root,
        "schemas/briefing-pack.schema.json",
        "/properties/coverage_audit/properties/slot_results/items/properties/status/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

fn ensure_impact_summary_enums_sync(root: &Path, sources: &ContractSources) -> Result<()> {
    ensure_schema_enum(
        root,
        "schemas/impact-summary.schema.json",
        "/properties/freshness_impact/enum",
        &sources.rust,
        &sources.ts,
        &sources.python,
    )
}

struct FieldMapping {
    schema_name: &'static str,
    rust_marker: &'static str,
    ts_marker: &'static str,
    python_marker: &'static str,
}

impl FieldMapping {
    const fn new(
        schema_name: &'static str,
        rust_marker: &'static str,
        ts_marker: &'static str,
        python_marker: &'static str,
    ) -> Self {
        Self {
            schema_name,
            rust_marker,
            ts_marker,
            python_marker,
        }
    }
}

fn ensure_schema_contract(
    root: &Path,
    schema_path: &str,
    field_mappings: &[FieldMapping],
    rust_source: &str,
    ts_source: &str,
    python_source: &str,
) -> Result<()> {
    let schema = load_schema(root, schema_path)?;
    let schema_properties = schema
        .get("properties")
        .and_then(Value::as_object)
        .with_context(|| format!("schema {schema_path} is missing an object `properties` block"))?;
    let mapped_properties = field_mappings
        .iter()
        .map(|mapping| mapping.schema_name.to_string())
        .collect::<BTreeSet<_>>();
    let actual_properties = schema_properties.keys().cloned().collect::<BTreeSet<_>>();

    if actual_properties != mapped_properties {
        bail!(
            "schema mapping drift in {schema_path}: schema properties {actual_properties:?} do not match sync mappings {mapped_properties:?}"
        );
    }

    for field in field_mappings {
        ensure_marker(
            schema_path,
            "Rust",
            field.schema_name,
            rust_source,
            field.rust_marker,
        )?;
        ensure_marker(
            schema_path,
            "TypeScript",
            field.schema_name,
            ts_source,
            field.ts_marker,
        )?;
        ensure_marker(
            schema_path,
            "Python",
            field.schema_name,
            python_source,
            field.python_marker,
        )?;
    }

    Ok(())
}

fn ensure_schema_enum(
    root: &Path,
    schema_path: &str,
    pointer: &str,
    rust_source: &str,
    ts_source: &str,
    python_source: &str,
) -> Result<()> {
    let schema = load_schema(root, schema_path)?;
    let values = schema
        .pointer(pointer)
        .and_then(Value::as_array)
        .with_context(|| format!("schema {schema_path} is missing enum at {pointer}"))?
        .iter()
        .map(|value| {
            value.as_str().with_context(|| {
                format!("schema {schema_path} enum at {pointer} contains a non-string value")
            })
        })
        .collect::<Result<Vec<_>>>()?;

    for value in values {
        let rust_variant = to_pascal_case(value);
        ensure_marker(schema_path, "Rust enum", value, rust_source, &rust_variant)?;
        ensure_marker(
            schema_path,
            "TypeScript enum",
            value,
            ts_source,
            &format!("\"{value}\""),
        )?;
        ensure_marker(
            schema_path,
            "Python enum",
            value,
            python_source,
            &format!("\"{value}\""),
        )?;
    }

    Ok(())
}

fn load_schema(root: &Path, schema_path: &str) -> Result<Value> {
    let schema_text = fs::read_to_string(root.join(schema_path))
        .with_context(|| format!("failed to read schema {}", root.join(schema_path).display()))?;
    serde_json::from_str(&schema_text).with_context(|| {
        format!(
            "failed to parse schema {}",
            root.join(schema_path).display()
        )
    })
}

fn ensure_marker(
    schema_path: &str,
    surface_name: &str,
    field_name: &str,
    source: &str,
    marker: &str,
) -> Result<()> {
    if !source.contains(marker) {
        bail!(
            "{surface_name} contract drift for {schema_path}: missing marker `{marker}` for schema item `{field_name}`"
        );
    }

    Ok(())
}

fn to_pascal_case(value: &str) -> String {
    value
        .split('_')
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => {
                    let mut normalized = String::new();
                    normalized.extend(first.to_uppercase());
                    normalized.push_str(chars.as_str());
                    normalized
                }
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn run(root: &Path, program: &str, args: &[&str], env_vars: &[(&str, String)]) -> Result<()> {
    run_in(root, program, args, env_vars)
}

fn run_in(dir: &Path, program: &str, args: &[&str], env_vars: &[(&str, String)]) -> Result<()> {
    println!("> {} {}", program, args.join(" "));

    let mut command = Command::new(resolve_program(program));
    command
        .current_dir(dir)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    for (key, value) in env_vars {
        command.env(key, value);
    }

    let status = command
        .status()
        .with_context(|| format!("failed to execute `{program}`"))?;

    if !status.success() {
        bail!("command failed: {} {}", program, args.join(" "));
    }

    Ok(())
}

fn resolve_program(program: &str) -> String {
    #[cfg(windows)]
    {
        if program.eq_ignore_ascii_case("pnpm") {
            return "pnpm.cmd".to_string();
        }
    }

    program.to_string()
}
