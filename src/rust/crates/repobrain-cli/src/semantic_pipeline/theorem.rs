use super::ivl::{IvlEvaluationAttempt, ivl_evaluate_obligation};
use super::l1::{first_non_empty_line, run_command_with_timeout, summarize_command_output};
use super::l2::{l2_align_functions, l2_changed_paths, l2_fnv1a_hash, l2_function_signals};
use super::{
    BTreeMap, BTreeSet, CliL2AlignmentMode, CliTheoremSolverMode, CliTranslationIrMode,
    CliTranslationRunStatusMode, CommandRunOutcome, Duration, EquivalenceEvidenceReceipt,
    EquivalenceEvidenceStatus, EquivalenceStage, Instant, L2FunctionSignal, L2RelationalPolicy,
    Path, PathBuf, ProcessCommand, RepositoryInventorySnapshot, Result, SemanticDelta,
    TheoremContract, TheoremEvaluation, TheoremExecutionContext, TheoremObligationStatus,
    TheoremPolicy, TheoremProofCertificate, TheoremProofObligation, TheoremReplayArtifact,
    TheoremReplayPolicy, TheoremRunStatus, TheoremStageArtifacts, fs, semantic_diff_summary,
};
use llvm_artifacts::theorem_collect_llvm_artifact_paths;
use rayon::{ThreadPoolBuilder, prelude::*};
use std::sync::Mutex;

mod llvm_artifacts;

#[derive(Debug, Clone)]
struct TheoremSignalCatalog {
    reference_signals: Vec<L2FunctionSignal>,
    target_signals: Vec<L2FunctionSignal>,
}

const THEOREM_MAX_PREWARM_MODULES: usize = 6;
const THEOREM_TOOL_PROBE_TIMEOUT_MS: u64 = 1_000;

pub(crate) fn theorem_stage_artifacts(
    repo_root: &Path,
    reference_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
    delta: &SemanticDelta,
    policy: &TheoremPolicy,
    contract: &TheoremContract,
) -> TheoremStageArtifacts {
    let changed_paths = l2_changed_paths(delta);
    let signal_catalog =
        theorem_signal_catalog(reference_snapshot, target_snapshot, &changed_paths, policy);
    let mut obligations = theorem_compile_obligations(&signal_catalog, policy, contract);
    if obligations.is_empty() {
        obligations = theorem_fallback_file_obligations(&changed_paths, policy, contract);
    }
    let obligations = theorem_schedule_obligations(obligations, policy);
    let context = theorem_execution_context(&signal_catalog);
    let compiler_resources = theorem_compile_resources(
        repo_root,
        Some(reference_snapshot),
        Some(target_snapshot),
        policy,
    );
    let certificates =
        theorem_execute_obligations(&obligations, &context, policy, &compiler_resources);
    let certificate_run_status = theorem_run_status_from_certificates(&certificates);
    let replay_artifact_path = if policy.persist_replay {
        theorem_persist_replay_artifact(
            repo_root,
            reference_snapshot,
            target_snapshot,
            contract,
            &obligations,
            &certificates,
            policy,
        )
    } else {
        None
    };
    let replay_command = replay_artifact_path
        .as_ref()
        .map(|path| theorem_replay_command(path, None));
    let mut extra_receipts = Vec::new();
    if let Some(receipt) = theorem_translation_validation_receipt_with_context(
        repo_root,
        Some(reference_snapshot),
        Some(target_snapshot),
        &obligations,
        Some(&context),
        &compiler_resources,
        policy,
    ) {
        extra_receipts.push(receipt);
    }
    let run_status =
        theorem_run_status_with_translation(certificate_run_status, &extra_receipts, policy);

    TheoremStageArtifacts {
        contract: Some(contract.clone()),
        run_status: Some(run_status),
        obligations,
        certificates,
        replay_artifact: replay_artifact_path,
        replay_command,
        extra_receipts,
    }
}

fn theorem_signal_catalog(
    reference_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
    changed_paths: &BTreeSet<String>,
    policy: &TheoremPolicy,
) -> TheoremSignalCatalog {
    TheoremSignalCatalog {
        reference_signals: l2_function_signals(
            reference_snapshot,
            changed_paths,
            policy.max_candidate_functions,
        ),
        target_signals: l2_function_signals(
            target_snapshot,
            changed_paths,
            policy.max_candidate_functions,
        ),
    }
}

fn theorem_compile_obligations(
    signal_catalog: &TheoremSignalCatalog,
    policy: &TheoremPolicy,
    contract: &TheoremContract,
) -> Vec<TheoremProofObligation> {
    let deadline = Instant::now() + Duration::from_millis(policy.timeout_ms);
    let alignment_policy = L2RelationalPolicy {
        max_pairs: policy.max_obligations,
        max_candidate_functions: policy.max_candidate_functions,
        timeout_ms: policy.timeout_ms,
        alignment_mode: CliL2AlignmentMode::SignatureAware,
        min_alignment_score: 70,
    };
    let alignment = l2_align_functions(
        &signal_catalog.reference_signals,
        &signal_catalog.target_signals,
        alignment_policy,
        deadline,
    );
    let mut obligations = Vec::<TheoremProofObligation>::new();
    let mut seen = BTreeSet::<String>::new();

    for pair in &alignment.pairs {
        let source_pair = format!("pair:{}=>{}", pair.reference.id, pair.target.id);
        let assumptions = vec![
            format!("alignment_strategy={}", pair.strategy),
            format!("alignment_score={}", pair.score),
        ];
        let obligation = theorem_obligation_for_source_pair(source_pair, assumptions, contract);
        if seen.insert(obligation.id.clone()) {
            obligations.push(obligation);
        }
        if obligations.len() >= policy.max_obligations {
            return obligations;
        }
    }

    for reference_only in &alignment.unmatched_reference {
        let source_pair = format!("reference_only:{}", reference_only.id);
        let obligation = theorem_obligation_for_source_pair(
            source_pair,
            vec!["target counterpart is missing under current alignment bounds".to_string()],
            contract,
        );
        if seen.insert(obligation.id.clone()) {
            obligations.push(obligation);
        }
        if obligations.len() >= policy.max_obligations {
            return obligations;
        }
    }
    for target_only in &alignment.unmatched_target {
        let source_pair = format!("target_only:{}", target_only.id);
        let obligation = theorem_obligation_for_source_pair(
            source_pair,
            vec!["reference counterpart is missing under current alignment bounds".to_string()],
            contract,
        );
        if seen.insert(obligation.id.clone()) {
            obligations.push(obligation);
        }
        if obligations.len() >= policy.max_obligations {
            return obligations;
        }
    }

    obligations
}

fn theorem_fallback_file_obligations(
    changed_paths: &BTreeSet<String>,
    policy: &TheoremPolicy,
    contract: &TheoremContract,
) -> Vec<TheoremProofObligation> {
    changed_paths
        .iter()
        .take(policy.max_obligations)
        .map(|path| {
            theorem_obligation_for_source_pair(
                format!("file:{path}"),
                vec!["function-level alignment produced no candidates".to_string()],
                contract,
            )
        })
        .collect()
}

pub(crate) fn theorem_obligation_for_source_pair(
    source_pair: String,
    assumptions: Vec<String>,
    contract: &TheoremContract,
) -> TheoremProofObligation {
    let mut obligation_assumptions = contract.environment_assumptions.clone();
    obligation_assumptions.extend(assumptions);
    let encoding_seed = format!(
        "contract={};pair={};assumptions={}",
        contract.contract_id,
        source_pair,
        obligation_assumptions.join("|")
    );
    let encoding_hash = format!("{:016x}", l2_fnv1a_hash(encoding_seed.as_bytes()));
    let id = format!(
        "thm_{:016x}",
        l2_fnv1a_hash(format!("{}|{}", contract.contract_id, encoding_hash).as_bytes())
    );

    TheoremProofObligation {
        id,
        source_pair,
        contract_id: contract.contract_id.clone(),
        assumptions: obligation_assumptions,
        encoding_hash,
    }
}

pub(crate) fn theorem_schedule_obligations(
    mut obligations: Vec<TheoremProofObligation>,
    policy: &TheoremPolicy,
) -> Vec<TheoremProofObligation> {
    obligations.sort_unstable_by(|left, right| {
        theorem_obligation_priority(&left.source_pair)
            .cmp(&theorem_obligation_priority(&right.source_pair))
            .then_with(|| {
                theorem_obligation_partition_key(&left.source_pair)
                    .cmp(&theorem_obligation_partition_key(&right.source_pair))
            })
            .then_with(|| left.id.cmp(&right.id))
    });

    obligations
        .into_iter()
        .take(policy.max_obligations)
        .map(|mut obligation| {
            obligation.assumptions.push(format!(
                "obligation_partition={}",
                theorem_obligation_partition_key(&obligation.source_pair)
            ));
            obligation.assumptions.push(format!(
                "obligation_priority={}",
                theorem_obligation_priority(&obligation.source_pair)
            ));
            obligation
        })
        .collect()
}

fn theorem_obligation_priority(source_pair: &str) -> u8 {
    if source_pair.starts_with("pair:") {
        return 1;
    }
    if source_pair.starts_with("reference_only:") || source_pair.starts_with("target_only:") {
        return 0;
    }
    if source_pair.starts_with("file:") {
        return 2;
    }

    3
}

fn theorem_obligation_partition_key(source_pair: &str) -> String {
    if let Some((reference_id, target_id)) = theorem_pair_ids(source_pair) {
        let mut parts = [
            theorem_signal_path_from_id(reference_id).unwrap_or_else(|| "unknown".to_string()),
            theorem_signal_path_from_id(target_id).unwrap_or_else(|| "unknown".to_string()),
        ];
        parts.sort_unstable();
        return format!("pair:{}=>{}", parts[0], parts[1]);
    }
    if let Some(reference_id) = source_pair.strip_prefix("reference_only:") {
        return format!(
            "reference:{}",
            theorem_signal_path_from_id(reference_id).unwrap_or_else(|| "unknown".to_string())
        );
    }
    if let Some(target_id) = source_pair.strip_prefix("target_only:") {
        return format!(
            "target:{}",
            theorem_signal_path_from_id(target_id).unwrap_or_else(|| "unknown".to_string())
        );
    }
    if let Some(path) = source_pair.strip_prefix("file:") {
        return format!("file:{path}");
    }

    "unknown".to_string()
}

fn theorem_execution_context(signal_catalog: &TheoremSignalCatalog) -> TheoremExecutionContext {
    let reference_map = signal_catalog
        .reference_signals
        .iter()
        .cloned()
        .map(|signal| (signal.id.clone(), signal))
        .collect::<BTreeMap<_, _>>();
    let target_map = signal_catalog
        .target_signals
        .iter()
        .cloned()
        .map(|signal| (signal.id.clone(), signal))
        .collect::<BTreeMap<_, _>>();

    TheoremExecutionContext {
        reference_map,
        target_map,
    }
}

fn theorem_execute_obligations(
    obligations: &[TheoremProofObligation],
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    compiler_resources: &TheoremCompileResources,
) -> Vec<TheoremProofCertificate> {
    if obligations.is_empty() {
        return Vec::new();
    }

    let deadline = Instant::now() + Duration::from_millis(policy.timeout_ms);
    let scheduled_timeout_ms = theorem_per_obligation_timeout_ms(
        policy.timeout_ms,
        obligations.len(),
        policy.max_parallelism,
    );
    let scheduled_timeout = Duration::from_millis(scheduled_timeout_ms);
    theorem_prewarm_compiler_ir_cache(obligations, context, policy, compiler_resources, deadline);
    let mut certificates = Vec::with_capacity(obligations.len());

    let parallelism = policy.max_parallelism.max(1);
    if parallelism == 1 || obligations.len() == 1 {
        for obligation in obligations {
            certificates.push(theorem_execute_scheduled_obligation(
                obligation,
                context,
                policy,
                deadline,
                scheduled_timeout,
                scheduled_timeout_ms,
                compiler_resources,
            ));
        }
        return certificates;
    }

    let pool = ThreadPoolBuilder::new().num_threads(parallelism).build();
    let Ok(pool) = pool else {
        for obligation in obligations {
            certificates.push(theorem_execute_scheduled_obligation(
                obligation,
                context,
                policy,
                deadline,
                scheduled_timeout,
                scheduled_timeout_ms,
                compiler_resources,
            ));
        }
        return certificates;
    };

    for chunk in obligations.chunks(parallelism) {
        let chunk_certificates = pool.install(|| {
            chunk
                .par_iter()
                .map(|obligation| {
                    theorem_execute_scheduled_obligation(
                        obligation,
                        context,
                        policy,
                        deadline,
                        scheduled_timeout,
                        scheduled_timeout_ms,
                        compiler_resources,
                    )
                })
                .collect::<Vec<_>>()
        });
        certificates.extend(chunk_certificates);
    }

    certificates
}

fn theorem_execute_scheduled_obligation(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    deadline: Instant,
    scheduled_timeout: Duration,
    reported_timeout_ms: u64,
    compiler_resources: &TheoremCompileResources,
) -> TheoremProofCertificate {
    let now = Instant::now();
    if now >= deadline {
        return theorem_inconclusive_certificate(
            obligation,
            Some("theorem_deadline_reached_before_obligation_execution".to_string()),
            Some("theorem_timeout".to_string()),
            Some(reported_timeout_ms),
            Some(theorem_solver_label(policy).to_string()),
        );
    }
    let remaining = deadline.saturating_duration_since(now);
    let timeout = remaining.min(scheduled_timeout);
    theorem_execute_single_obligation(
        obligation,
        context,
        policy,
        timeout,
        reported_timeout_ms,
        compiler_resources,
    )
}

fn theorem_per_obligation_timeout_ms(
    total_timeout_ms: u64,
    obligation_count: usize,
    max_parallelism: usize,
) -> u64 {
    let obligations = u64::try_from(obligation_count.max(1)).unwrap_or(1);
    let parallelism = u64::try_from(max_parallelism.max(1)).unwrap_or(1);
    let slots = obligations.div_ceil(parallelism).max(1);
    let fair_share = total_timeout_ms / slots;
    fair_share.max(400)
}

fn theorem_execute_single_obligation(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    timeout: Duration,
    reported_timeout_ms: u64,
    compiler_resources: &TheoremCompileResources,
) -> TheoremProofCertificate {
    let started = Instant::now();
    let solver_timeout = timeout.min(Duration::from_millis(policy.solver_timeout_ms));
    let mut certificate = theorem_stability_checked_certificate(
        obligation,
        context,
        policy,
        solver_timeout,
        compiler_resources,
    );
    let mut retries = policy.flaky_retries.saturating_sub(1);

    while retries > 0
        && matches!(certificate.status, TheoremObligationStatus::Inconclusive)
        && started.elapsed() < timeout
    {
        let retry = theorem_stability_checked_certificate(
            obligation,
            context,
            policy,
            solver_timeout,
            compiler_resources,
        );
        if !matches!(retry.status, TheoremObligationStatus::Inconclusive) {
            certificate = retry;
            break;
        }
        retries -= 1;
    }

    if started.elapsed() >= timeout {
        return theorem_inconclusive_certificate(
            obligation,
            Some("theorem_obligation_timeout".to_string()),
            Some(format!("theorem_obligation_timeout:{}", obligation.id)),
            Some(reported_timeout_ms),
            Some(theorem_solver_label(policy).to_string()),
        );
    }

    certificate.timeout_ms = Some(reported_timeout_ms);
    certificate
}

fn theorem_stability_checked_certificate(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    solver_timeout: Duration,
    compiler_resources: &TheoremCompileResources,
) -> TheoremProofCertificate {
    let first = theorem_evaluate_obligation(
        obligation,
        context,
        policy,
        solver_timeout,
        compiler_resources,
    );
    for _ in 1..policy.stability_runs {
        let current = theorem_evaluate_obligation(
            obligation,
            context,
            policy,
            solver_timeout,
            compiler_resources,
        );
        if current.status != first.status || current.witness != first.witness {
            return theorem_inconclusive_certificate(
                obligation,
                Some("flaky_outcome_detected_across_stability_runs".to_string()),
                Some(format!("theorem_flaky_outcome:{}", obligation.id)),
                Some(policy.timeout_ms),
                Some(theorem_solver_label(policy).to_string()),
            );
        }
    }

    TheoremProofCertificate {
        obligation_id: obligation.id.clone(),
        encoding_hash: Some(obligation.encoding_hash.clone()),
        status: first.status,
        solver: first.solver,
        timeout_ms: Some(policy.timeout_ms),
        assumptions: first.assumptions,
        witness: first.witness,
    }
}

fn theorem_evaluate_obligation(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    solver_timeout: Duration,
    compiler_resources: &TheoremCompileResources,
) -> TheoremEvaluation {
    if matches!(policy.solver_mode, CliTheoremSolverMode::SmtZ3) {
        if let Some((reference_id, target_id)) = theorem_pair_ids(&obligation.source_pair)
            && let (Some(reference_signal), Some(target_signal)) = (
                context.reference_map.get(reference_id),
                context.target_map.get(target_id),
            )
        {
            let compiler_ir = theorem_compiler_emitted_pair_ir(
                &compiler_resources.workspaces,
                &compiler_resources.ir_cache,
                reference_signal,
                target_signal,
                solver_timeout,
            );
            match compiler_ir {
                Ok((reference_ir, target_ir)) => {
                    match ivl_evaluate_obligation(
                        obligation,
                        &reference_ir,
                        &target_ir,
                        policy,
                        solver_timeout,
                    ) {
                        IvlEvaluationAttempt::Completed(evaluation) => return evaluation,
                        IvlEvaluationAttempt::Unavailable { reason } => {
                            let mut fallback = theorem_evaluate_obligation_smt(
                                obligation,
                                context,
                                policy,
                                solver_timeout,
                            );
                            fallback.assumptions.push(format!(
                                "LLVM SSA/IVL backend was unavailable (`{reason}`); falling back to bounded signal-model solver lane"
                            ));
                            return fallback;
                        }
                    }
                }
                Err(reason) => {
                    let mut fallback = theorem_evaluate_obligation_smt(
                        obligation,
                        context,
                        policy,
                        solver_timeout,
                    );
                    fallback.assumptions.push(format!(
                        "compiler-emitted LLVM IR was unavailable (`{reason}`); falling back to bounded signal-model solver lane"
                    ));
                    return fallback;
                }
            }
        }
        return theorem_evaluate_obligation_smt(obligation, context, policy, solver_timeout);
    }

    theorem_evaluate_obligation_relational(obligation, context)
}

fn theorem_evaluate_obligation_relational(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
) -> TheoremEvaluation {
    if let Some((reference_id, target_id)) = theorem_pair_ids(&obligation.source_pair) {
        let Some(reference_signal) = context.reference_map.get(reference_id) else {
            return TheoremEvaluation {
                status: TheoremObligationStatus::Inconclusive,
                solver: Some("theorem_relational_solver_v1".to_string()),
                assumptions: vec![
                    "reference signal not found for aligned theorem pair".to_string(),
                ],
                witness: Some(format!("theorem_missing_reference_signal:{reference_id}")),
            };
        };
        let Some(target_signal) = context.target_map.get(target_id) else {
            return TheoremEvaluation {
                status: TheoremObligationStatus::Inconclusive,
                solver: Some("theorem_relational_solver_v1".to_string()),
                assumptions: vec!["target signal not found for aligned theorem pair".to_string()],
                witness: Some(format!("theorem_missing_target_signal:{target_id}")),
            };
        };
        let reasons = theorem_pair_difference_reasons(reference_signal, target_signal);
        if reasons.is_empty() {
            return TheoremEvaluation {
                status: TheoremObligationStatus::Proved,
                solver: Some("theorem_relational_solver_v1".to_string()),
                assumptions: vec![
                    "pair-level relational signatures match under theorem contract bounds"
                        .to_string(),
                ],
                witness: None,
            };
        }

        return TheoremEvaluation {
            status: TheoremObligationStatus::Refuted,
            solver: Some("theorem_relational_solver_v1".to_string()),
            assumptions: vec![
                "pair-level relational signatures diverged under theorem contract bounds"
                    .to_string(),
            ],
            witness: Some(format!(
                "theorem_pair_counterexample:{}=>{}:{}",
                reference_signal.id,
                target_signal.id,
                reasons.join(",")
            )),
        };
    }

    if let Some(reference_id) = obligation.source_pair.strip_prefix("reference_only:") {
        return TheoremEvaluation {
            status: TheoremObligationStatus::Refuted,
            solver: Some("theorem_relational_solver_v1".to_string()),
            assumptions: vec!["target counterpart missing under bounded alignment".to_string()],
            witness: Some(format!("theorem_unmatched_reference:{reference_id}")),
        };
    }
    if let Some(target_id) = obligation.source_pair.strip_prefix("target_only:") {
        return TheoremEvaluation {
            status: TheoremObligationStatus::Refuted,
            solver: Some("theorem_relational_solver_v1".to_string()),
            assumptions: vec!["reference counterpart missing under bounded alignment".to_string()],
            witness: Some(format!("theorem_unmatched_target:{target_id}")),
        };
    }
    if let Some(path) = obligation.source_pair.strip_prefix("file:") {
        return TheoremEvaluation {
            status: TheoremObligationStatus::Inconclusive,
            solver: Some("theorem_relational_solver_v1".to_string()),
            assumptions: vec![
                "file-only fallback obligation does not carry enough aligned semantic structure"
                    .to_string(),
            ],
            witness: Some(format!("theorem_file_scope_inconclusive:{path}")),
        };
    }

    TheoremEvaluation {
        status: TheoremObligationStatus::Inconclusive,
        solver: Some("theorem_relational_solver_v1".to_string()),
        assumptions: vec!["unrecognized theorem obligation source_pair format".to_string()],
        witness: Some(format!(
            "theorem_unrecognized_source_pair:{}",
            obligation.source_pair
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtCheckResult {
    Sat,
    Unsat,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TheoremSmtFieldConstraint {
    symbol: &'static str,
    reference_hash: u64,
    target_hash: u64,
    weight: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TheoremSmtPairModel {
    fields: Vec<TheoremSmtFieldConstraint>,
    min_score: u16,
    line_close_cap: u32,
    line_close_weight: u16,
    reference_line: u32,
    target_line: u32,
    obligation_contract_hash: u64,
    obligation_source_pair_hash: u64,
    obligation_assumptions_hash: u64,
    obligation_encoding_hash: u64,
    obligation_assumption_count: u32,
}

fn theorem_evaluate_obligation_smt(
    obligation: &TheoremProofObligation,
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    solver_timeout: Duration,
) -> TheoremEvaluation {
    let Some((reference_id, target_id)) = theorem_pair_ids(&obligation.source_pair) else {
        return theorem_evaluate_obligation_relational(obligation, context);
    };
    let Some(reference_signal) = context.reference_map.get(reference_id) else {
        return theorem_evaluate_obligation_relational(obligation, context);
    };
    let Some(target_signal) = context.target_map.get(target_id) else {
        return theorem_evaluate_obligation_relational(obligation, context);
    };

    let smt_model = theorem_smt_pair_model(obligation, reference_signal, target_signal);
    let semantic_score = theorem_pair_smt_score(&smt_model);
    let mismatch_reasons = theorem_pair_difference_reasons(reference_signal, target_signal);
    let (smt_outcome, used_ground_evaluator) =
        if let Some(outcome) = theorem_ground_model_outcome(&smt_model) {
            (outcome, true)
        } else {
            let script = theorem_smt_script(&smt_model);
            let run = run_theorem_smt_solver(policy, obligation, &script, solver_timeout);
            (theorem_smt_outcome(run, &mismatch_reasons), false)
        };
    let solver_name = if used_ground_evaluator {
        "theorem_symbolic_ground_evaluator_v1"
    } else {
        "z3_smtlib2"
    };
    match smt_outcome {
        SmtCheckResult::Unsat => TheoremEvaluation {
            status: TheoremObligationStatus::Proved,
            solver: Some(solver_name.to_string()),
            assumptions: vec![
                "SMT backend checked negated bounded-equivalence formula and returned unsat"
                    .to_string(),
                format!(
                    "weighted symbolic constraints over obligation + signal fields satisfied under score>={} (observed_score={semantic_score})",
                    smt_model.min_score
                ),
                if used_ground_evaluator {
                    "ground symbolic evaluator resolved obligation without external solver spawn"
                        .to_string()
                } else {
                    "external SMT solver execution path was used".to_string()
                },
            ],
            witness: None,
        },
        SmtCheckResult::Sat => TheoremEvaluation {
            status: TheoremObligationStatus::Refuted,
            solver: Some(solver_name.to_string()),
            assumptions: vec![
                "SMT backend found a satisfiable negated bounded-equivalence formula".to_string(),
                format!(
                    "counterexample lane is bounded to weighted obligation-field model (observed_score={semantic_score}, required_min={})",
                    smt_model.min_score
                ),
                if used_ground_evaluator {
                    "ground symbolic evaluator resolved obligation without external solver spawn"
                        .to_string()
                } else {
                    "external SMT solver execution path was used".to_string()
                },
            ],
            witness: Some(format!(
                "theorem_pair_counterexample:{}=>{}:score={semantic_score}/{}:{}",
                reference_signal.id,
                target_signal.id,
                smt_model.min_score,
                mismatch_reasons.join(",")
            )),
        },
        SmtCheckResult::Unknown => {
            if policy.solver_fallback_relational {
                let mut fallback = theorem_evaluate_obligation_relational(obligation, context);
                fallback.assumptions.push(
                    "SMT backend was unavailable/inconclusive; falling back to deterministic relational solver lane".to_string(),
                );
                return fallback;
            }

            TheoremEvaluation {
                status: TheoremObligationStatus::Inconclusive,
                solver: Some(solver_name.to_string()),
                assumptions: vec![
                    "SMT backend did not produce sat/unsat under configured timeout".to_string(),
                    "solver fallback to relational lane is disabled".to_string(),
                ],
                witness: Some(format!("theorem_smt_inconclusive:{}", obligation.id)),
            }
        }
    }
}

fn theorem_smt_pair_model(
    obligation: &TheoremProofObligation,
    reference: &L2FunctionSignal,
    target: &L2FunctionSignal,
) -> TheoremSmtPairModel {
    let assumptions_hash = l2_fnv1a_hash(obligation.assumptions.join("|").as_bytes());
    let encoding_hash = u64::from_str_radix(obligation.encoding_hash.trim(), 16)
        .unwrap_or_else(|_| l2_fnv1a_hash(obligation.encoding_hash.as_bytes()));

    let fields = vec![
        TheoremSmtFieldConstraint {
            symbol: "declaration",
            reference_hash: l2_fnv1a_hash(
                reference.behavior_profile.declaration_signature.as_bytes(),
            ),
            target_hash: l2_fnv1a_hash(target.behavior_profile.declaration_signature.as_bytes()),
            weight: 24,
        },
        TheoremSmtFieldConstraint {
            symbol: "call",
            reference_hash: l2_fnv1a_hash(reference.behavior_profile.call_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.behavior_profile.call_signature.as_bytes()),
            weight: 18,
        },
        TheoremSmtFieldConstraint {
            symbol: "control",
            reference_hash: l2_fnv1a_hash(reference.behavior_profile.control_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.behavior_profile.control_signature.as_bytes()),
            weight: 10,
        },
        TheoremSmtFieldConstraint {
            symbol: "literal",
            reference_hash: l2_fnv1a_hash(reference.behavior_profile.literal_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.behavior_profile.literal_signature.as_bytes()),
            weight: 8,
        },
        TheoremSmtFieldConstraint {
            symbol: "window_hash",
            reference_hash: l2_fnv1a_hash(
                reference.behavior_profile.normalized_window_hash.as_bytes(),
            ),
            target_hash: l2_fnv1a_hash(target.behavior_profile.normalized_window_hash.as_bytes()),
            weight: 16,
        },
        TheoremSmtFieldConstraint {
            symbol: "import",
            reference_hash: l2_fnv1a_hash(reference.import_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.import_signature.as_bytes()),
            weight: 10,
        },
        TheoremSmtFieldConstraint {
            symbol: "reverse_import",
            reference_hash: l2_fnv1a_hash(reference.reverse_import_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.reverse_import_signature.as_bytes()),
            weight: 4,
        },
        TheoremSmtFieldConstraint {
            symbol: "symbol_context",
            reference_hash: l2_fnv1a_hash(reference.symbol_context_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.symbol_context_signature.as_bytes()),
            weight: 8,
        },
        TheoremSmtFieldConstraint {
            symbol: "behavior_signature",
            reference_hash: l2_fnv1a_hash(reference.behavior_signature.as_bytes()),
            target_hash: l2_fnv1a_hash(target.behavior_signature.as_bytes()),
            weight: 12,
        },
        TheoremSmtFieldConstraint {
            symbol: "name_key",
            reference_hash: l2_fnv1a_hash(reference.name_key.as_bytes()),
            target_hash: l2_fnv1a_hash(target.name_key.as_bytes()),
            weight: 6,
        },
    ];

    TheoremSmtPairModel {
        fields,
        min_score: 78,
        line_close_cap: 12,
        line_close_weight: 4,
        reference_line: reference.line_number,
        target_line: target.line_number,
        obligation_contract_hash: l2_fnv1a_hash(obligation.contract_id.as_bytes()),
        obligation_source_pair_hash: l2_fnv1a_hash(obligation.source_pair.as_bytes()),
        obligation_assumptions_hash: assumptions_hash,
        obligation_encoding_hash: encoding_hash,
        obligation_assumption_count: u32::try_from(obligation.assumptions.len())
            .unwrap_or(u32::MAX),
    }
}

fn theorem_pair_smt_score(model: &TheoremSmtPairModel) -> u16 {
    let mut score = 0_u16;
    for field in &model.fields {
        if field.reference_hash == field.target_hash {
            score = score.saturating_add(field.weight);
        }
    }
    if model.reference_line.abs_diff(model.target_line) <= model.line_close_cap {
        score = score.saturating_add(model.line_close_weight);
    }

    score
}

fn theorem_ground_model_outcome(model: &TheoremSmtPairModel) -> Option<SmtCheckResult> {
    if !theorem_smt_model_is_ground(model) {
        return None;
    }
    if theorem_smt_equivalence_ok(model) {
        return Some(SmtCheckResult::Unsat);
    }

    Some(SmtCheckResult::Sat)
}

fn theorem_smt_model_is_ground(model: &TheoremSmtPairModel) -> bool {
    !model.fields.is_empty()
}

fn theorem_smt_equivalence_ok(model: &TheoremSmtPairModel) -> bool {
    let obligation_fields_bound = model.obligation_contract_hash > 0
        && model.obligation_source_pair_hash > 0
        && model.obligation_assumptions_hash > 0
        && model.obligation_encoding_hash > 0;
    let declaration_eq = theorem_smt_field_equal(model, "declaration");
    let window_hash_eq = theorem_smt_field_equal(model, "window_hash");
    let behavior_signature_eq = theorem_smt_field_equal(model, "behavior_signature");
    let import_eq = theorem_smt_field_equal(model, "import");
    let reverse_import_eq = theorem_smt_field_equal(model, "reverse_import");
    let line_close = model.reference_line.abs_diff(model.target_line) <= model.line_close_cap;

    obligation_fields_bound
        && theorem_pair_smt_score(model) >= model.min_score
        && declaration_eq
        && window_hash_eq
        && behavior_signature_eq
        && (import_eq || reverse_import_eq)
        && line_close
}

fn theorem_smt_field_equal(model: &TheoremSmtPairModel, symbol: &str) -> bool {
    model
        .fields
        .iter()
        .find(|field| field.symbol == symbol)
        .is_some_and(|field| field.reference_hash == field.target_hash)
}

fn theorem_smt_script(model: &TheoremSmtPairModel) -> String {
    let mut lines = vec![
        "(set-logic QF_LIA)".to_string(),
        "(set-option :produce-models true)".to_string(),
    ];

    lines.push("(declare-const contract_tag Int)".to_string());
    lines.push(format!(
        "(assert (= contract_tag {}))",
        model.obligation_contract_hash
    ));
    lines.push("(declare-const source_pair_tag Int)".to_string());
    lines.push(format!(
        "(assert (= source_pair_tag {}))",
        model.obligation_source_pair_hash
    ));
    lines.push("(declare-const assumptions_tag Int)".to_string());
    lines.push(format!(
        "(assert (= assumptions_tag {}))",
        model.obligation_assumptions_hash
    ));
    lines.push("(declare-const encoding_tag Int)".to_string());
    lines.push(format!(
        "(assert (= encoding_tag {}))",
        model.obligation_encoding_hash
    ));
    lines.push("(declare-const assumption_count Int)".to_string());
    lines.push(format!(
        "(assert (= assumption_count {}))",
        model.obligation_assumption_count
    ));
    lines.push(
        "(define-fun obligation_fields_bound () Bool (and (> contract_tag 0) (> source_pair_tag 0) (> assumptions_tag 0) (> encoding_tag 0) (>= assumption_count 0)))"
            .to_string(),
    );

    for field in &model.fields {
        let left_symbol = format!("ref_{}_tag", field.symbol);
        let right_symbol = format!("tgt_{}_tag", field.symbol);
        let eq_symbol = format!("{}_eq", field.symbol);
        lines.push(format!("(declare-const {left_symbol} Int)"));
        lines.push(format!(
            "(assert (= {left_symbol} {}))",
            field.reference_hash
        ));
        lines.push(format!("(declare-const {right_symbol} Int)"));
        lines.push(format!("(assert (= {right_symbol} {}))", field.target_hash));
        lines.push(format!(
            "(define-fun {eq_symbol} () Bool (= {left_symbol} {right_symbol}))"
        ));
    }

    lines.push("(declare-const ref_line Int)".to_string());
    lines.push(format!("(assert (= ref_line {}))", model.reference_line));
    lines.push("(declare-const tgt_line Int)".to_string());
    lines.push(format!("(assert (= tgt_line {}))", model.target_line));
    lines.push(
        "(define-fun line_delta () Int (ite (>= ref_line tgt_line) (- ref_line tgt_line) (- tgt_line ref_line)))"
            .to_string(),
    );
    lines.push(format!(
        "(define-fun line_close () Bool (<= line_delta {}))",
        model.line_close_cap
    ));

    let mut score_terms = model
        .fields
        .iter()
        .map(|field| format!("(ite {}_eq {} 0)", field.symbol, field.weight))
        .collect::<Vec<_>>();
    score_terms.push(format!("(ite line_close {} 0)", model.line_close_weight));
    lines.push(format!(
        "(define-fun semantic_score () Int (+ {}))",
        score_terms.join(" ")
    ));
    lines.push(format!(
        "(define-fun equivalence_ok () Bool (and obligation_fields_bound (>= semantic_score {}) declaration_eq window_hash_eq behavior_signature_eq (or import_eq reverse_import_eq) line_close))",
        model.min_score
    ));
    lines.push("(assert (not equivalence_ok))".to_string());
    lines.push("(check-sat)".to_string());
    lines.push("(get-model)".to_string());
    lines.join("\n")
}

fn run_theorem_smt_solver(
    policy: &TheoremPolicy,
    obligation: &TheoremProofObligation,
    script: &str,
    timeout: Duration,
) -> CommandRunOutcome {
    let solver_binary = policy.solver_path.as_ref().map_or_else(
        || "z3".to_string(),
        |path| path.to_string_lossy().to_string(),
    );
    let script_path = std::env::temp_dir().join(format!("repobrain-{}.smt2", obligation.id));
    if fs::write(&script_path, script).is_err() {
        return CommandRunOutcome::IoError("failed to persist theorem SMT script".to_string());
    }

    let mut command = ProcessCommand::new(&solver_binary);
    match policy.solver_mode {
        CliTheoremSolverMode::RelationalHeuristic => {
            return CommandRunOutcome::IoError(
                "invalid solver mode for SMT solver execution".to_string(),
            );
        }
        CliTheoremSolverMode::SmtZ3 => {
            let timeout_secs = timeout.as_secs().max(1);
            command.arg("-smt2");
            command.arg(format!("-T:{timeout_secs}"));
            command.arg(&script_path);
        }
    }

    let run = run_command_with_timeout(command, timeout);
    let _ = fs::remove_file(script_path);
    run
}

fn theorem_smt_outcome(
    run: CommandRunOutcome,
    mismatch_reasons: &[&'static str],
) -> SmtCheckResult {
    let mismatch_hint = mismatch_reasons.join(",");
    match run {
        CommandRunOutcome::Completed(output) => {
            let stdout = summarize_command_output(&output.stdout);
            let stderr = summarize_command_output(&output.stderr);
            let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
            if combined.contains("unsat") {
                return SmtCheckResult::Unsat;
            }
            if combined.contains("sat") {
                return SmtCheckResult::Sat;
            }
            if combined.contains("unknown") || combined.contains("timeout") {
                return SmtCheckResult::Unknown;
            }
            if output.status.success() && mismatch_hint.is_empty() {
                return SmtCheckResult::Unsat;
            }
            if output.status.success() && !mismatch_hint.is_empty() {
                return SmtCheckResult::Sat;
            }
            SmtCheckResult::Unknown
        }
        CommandRunOutcome::TimedOut(_) => SmtCheckResult::Unknown,
        CommandRunOutcome::SpawnFailed(_) | CommandRunOutcome::IoError(_) => {
            SmtCheckResult::Unknown
        }
    }
}

fn theorem_solver_label(policy: &TheoremPolicy) -> &'static str {
    match policy.solver_mode {
        CliTheoremSolverMode::RelationalHeuristic => "theorem_relational_solver_v1",
        CliTheoremSolverMode::SmtZ3 => "theorem_llvm_ssa_bv_v1",
    }
}

fn theorem_pair_difference_reasons(
    reference: &L2FunctionSignal,
    target: &L2FunctionSignal,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if reference.behavior_profile.declaration_signature
        != target.behavior_profile.declaration_signature
    {
        reasons.push("declaration_signature");
    }
    if reference.behavior_profile.call_signature != target.behavior_profile.call_signature {
        reasons.push("call_signature");
    }
    if reference.behavior_profile.control_signature != target.behavior_profile.control_signature {
        reasons.push("control_signature");
    }
    if reference.behavior_profile.literal_signature != target.behavior_profile.literal_signature {
        reasons.push("literal_signature");
    }
    if reference.behavior_profile.normalized_window_hash
        != target.behavior_profile.normalized_window_hash
    {
        reasons.push("window_hash");
    }
    if reference.import_signature != target.import_signature {
        reasons.push("import_signature");
    }
    if reference.symbol_context_signature != target.symbol_context_signature {
        reasons.push("symbol_context_signature");
    }
    if reference.reverse_import_signature != target.reverse_import_signature {
        reasons.push("reverse_import_signature");
    }
    if reference.behavior_signature != target.behavior_signature {
        reasons.push("behavior_signature");
    }
    if reference.line_number.abs_diff(target.line_number) > 12 {
        reasons.push("line_distance");
    }

    reasons
}

fn theorem_pair_ids(source_pair: &str) -> Option<(&str, &str)> {
    let payload = source_pair.strip_prefix("pair:")?;
    payload.split_once("=>")
}

fn theorem_signal_path_from_id(signal_id: &str) -> Option<String> {
    let (_, path_with_line) = signal_id.split_once('@')?;
    let (path, _) = path_with_line.rsplit_once(':')?;
    Some(path.to_string())
}

fn theorem_inconclusive_certificate(
    obligation: &TheoremProofObligation,
    reason: Option<String>,
    witness: Option<String>,
    timeout_ms: Option<u64>,
    solver: Option<String>,
) -> TheoremProofCertificate {
    let mut assumptions = Vec::new();
    if let Some(reason) = reason {
        assumptions.push(reason);
    }
    assumptions.push(format!(
        "obligation_encoding_hash={}",
        obligation.encoding_hash
    ));

    TheoremProofCertificate {
        obligation_id: obligation.id.clone(),
        encoding_hash: Some(obligation.encoding_hash.clone()),
        status: TheoremObligationStatus::Inconclusive,
        solver,
        timeout_ms,
        assumptions,
        witness,
    }
}

fn theorem_persist_replay_artifact(
    repo_root: &Path,
    reference_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
    contract: &TheoremContract,
    obligations: &[TheoremProofObligation],
    certificates: &[TheoremProofCertificate],
    policy: &TheoremPolicy,
) -> Option<String> {
    let artifact = TheoremReplayArtifact {
        artifact_version: "v1".to_string(),
        snapshot_id: target_snapshot.snapshot_id.clone(),
        reference_snapshot_id: reference_snapshot.snapshot_id.clone(),
        contract_id: contract.contract_id.clone(),
        policy: TheoremReplayPolicy {
            max_obligations: policy.max_obligations,
            max_candidate_functions: policy.max_candidate_functions,
            timeout_ms: policy.timeout_ms,
            flaky_retries: policy.flaky_retries,
            stability_runs: policy.stability_runs,
            max_parallelism: policy.max_parallelism,
        },
        obligations: obligations.to_vec(),
        certificates: certificates.to_vec(),
    };
    let path = match policy.replay_out.as_ref() {
        Some(explicit_path) => explicit_path.clone(),
        None => theorem_default_replay_path(
            repo_root,
            &reference_snapshot.snapshot_id,
            &target_snapshot.snapshot_id,
        ),
    };
    if let Some(parent) = path.parent()
        && fs::create_dir_all(parent).is_err()
    {
        return None;
    }
    let Ok(serialized) = serde_json::to_string_pretty(&artifact) else {
        return None;
    };
    if fs::write(&path, serialized).is_err() {
        return None;
    }

    Some(path.to_string_lossy().to_string())
}

fn theorem_default_replay_path(
    repo_root: &Path,
    reference_snapshot_id: &str,
    target_snapshot_id: &str,
) -> PathBuf {
    let run_seed = format!(
        "{}|{}|{}",
        reference_snapshot_id,
        target_snapshot_id,
        unique_run_nonce()
    );
    let run_id = format!("{:016x}", l2_fnv1a_hash(run_seed.as_bytes()));
    repo_root
        .join(".repobrain")
        .join("theorem-runs")
        .join(format!("{run_id}.json"))
}

fn unique_run_nonce() -> u128 {
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(error) => error.duration().as_nanos(),
    }
}

fn theorem_replay_command(path: &str, obligation_id: Option<&str>) -> String {
    let escaped_path = path.replace('\\', "\\\\");
    match obligation_id {
        Some(obligation_id) => {
            format!(
                "repobrain replay-theorem --artifact \"{escaped_path}\" --obligation-id {obligation_id}"
            )
        }
        None => format!("repobrain replay-theorem --artifact \"{escaped_path}\""),
    }
}

#[cfg(test)]
pub(crate) fn theorem_translation_validation_receipt(
    repo_root: &Path,
    obligations: &[TheoremProofObligation],
    policy: &TheoremPolicy,
) -> Option<EquivalenceEvidenceReceipt> {
    let compiler_resources = theorem_compile_resources(repo_root, None, None, policy);
    theorem_translation_validation_receipt_with_context(
        repo_root,
        None,
        None,
        obligations,
        None,
        &compiler_resources,
        policy,
    )
}

fn theorem_translation_validation_receipt_with_context(
    repo_root: &Path,
    _reference_snapshot: Option<&RepositoryInventorySnapshot>,
    _target_snapshot: Option<&RepositoryInventorySnapshot>,
    obligations: &[TheoremProofObligation],
    context: Option<&TheoremExecutionContext>,
    compiler_resources: &TheoremCompileResources,
    policy: &TheoremPolicy,
) -> Option<EquivalenceEvidenceReceipt> {
    if !policy.translation_validation {
        return None;
    }

    let alive2_binary = policy.alive2_path.as_ref().map_or_else(
        || "alive-tv".to_string(),
        |path| path.to_string_lossy().to_string(),
    );
    let pair_obligations = theorem_pair_obligations_for_translation(
        obligations,
        context,
        policy.translation_max_obligations,
    );
    if pair_obligations.is_empty() {
        return Some(theorem_translation_no_pairs_receipt(policy));
    }
    if let Err(reason) = theorem_probe_external_tool(
        &alive2_binary,
        repo_root,
        Duration::from_millis(THEOREM_TOOL_PROBE_TIMEOUT_MS.min(policy.translation_timeout_ms)),
    ) {
        return Some(theorem_translation_tool_unavailable_receipt(
            policy,
            &alive2_binary,
            &reason,
        ));
    }

    let per_obligation_timeout = theorem_translation_per_obligation_timeout(
        policy.translation_timeout_ms,
        pair_obligations.len(),
    );
    let checked_obligations = pair_obligations.len();
    let mut stats = TranslationRunStats::default();
    for obligation in pair_obligations {
        let (obligation_outcome, candidate_witness, used_compiler_ir) =
            theorem_translation_run_obligation(
                repo_root,
                context,
                compiler_resources,
                obligation,
                &alive2_binary,
                per_obligation_timeout,
            );
        if used_compiler_ir {
            stats.compiler_ir_runs += 1;
        } else {
            stats.abstract_fallback_runs += 1;
        }
        theorem_translation_record_outcome(&mut stats, obligation_outcome, candidate_witness);
    }

    Some(theorem_translation_receipt_from_stats(
        policy,
        stats,
        compiler_resources.workspaces.reference_reason.as_ref(),
        checked_obligations,
    ))
}

#[derive(Debug, Default)]
struct TranslationRunStats {
    proved: usize,
    refuted: usize,
    inconclusive: usize,
    compiler_ir_runs: usize,
    abstract_fallback_runs: usize,
    witness: Option<String>,
}

#[derive(Debug)]
struct TheoremCompileResources {
    workspaces: TranslationWorkspaceSet,
    ir_cache: Mutex<TranslationIrCache>,
}

fn theorem_compile_resources(
    repo_root: &Path,
    reference_snapshot: Option<&RepositoryInventorySnapshot>,
    target_snapshot: Option<&RepositoryInventorySnapshot>,
    policy: &TheoremPolicy,
) -> TheoremCompileResources {
    TheoremCompileResources {
        workspaces: theorem_translation_workspaces(
            repo_root,
            reference_snapshot,
            target_snapshot,
            policy,
        ),
        ir_cache: Mutex::new(TranslationIrCache::default()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct TranslationModuleKey {
    workspace_root: String,
    manifest_path: String,
}

#[derive(Debug, Default)]
struct TranslationIrCache {
    modules: BTreeMap<TranslationModuleKey, Result<Vec<ExtractedLlvmFunction>, String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractedLlvmFunction {
    symbol_name: String,
    subprogram_name: Option<String>,
    source_path: Option<String>,
    line_number: Option<u32>,
    ir: String,
}

#[derive(Debug)]
struct TranslationWorkspaceSet {
    reference_root: PathBuf,
    target_root: PathBuf,
    shared_target_dir: PathBuf,
    reference_reason: Option<String>,
    reference_checkout: Option<GitWorktreeCheckout>,
}

#[derive(Debug)]
struct GitWorktreeCheckout {
    repo_root: PathBuf,
    path: PathBuf,
    cleanup_on_drop: bool,
}

impl Drop for GitWorktreeCheckout {
    fn drop(&mut self) {
        if !self.cleanup_on_drop {
            return;
        }
        let mut command = ProcessCommand::new("git");
        command.arg("-C");
        command.arg(&self.repo_root);
        command.arg("worktree");
        command.arg("remove");
        command.arg("--force");
        command.arg(&self.path);
        let _ = command.output();
    }
}

fn theorem_pair_obligations_for_translation<'a>(
    obligations: &'a [TheoremProofObligation],
    context: Option<&TheoremExecutionContext>,
    max_obligations: usize,
) -> Vec<&'a TheoremProofObligation> {
    let mut pair_obligations = obligations
        .iter()
        .filter(|obligation| theorem_pair_ids(&obligation.source_pair).is_some())
        .collect::<Vec<_>>();
    pair_obligations.sort_unstable_by(|left, right| {
        theorem_pair_translation_readiness(right, context)
            .cmp(&theorem_pair_translation_readiness(left, context))
            .then_with(|| {
                theorem_obligation_strategy_rank(right).cmp(&theorem_obligation_strategy_rank(left))
            })
            .then_with(|| {
                theorem_obligation_alignment_score(right)
                    .cmp(&theorem_obligation_alignment_score(left))
            })
            .then_with(|| left.source_pair.cmp(&right.source_pair))
            .then_with(|| left.id.cmp(&right.id))
    });
    pair_obligations.truncate(max_obligations);
    pair_obligations
}

fn theorem_pair_translation_readiness(
    obligation: &TheoremProofObligation,
    context: Option<&TheoremExecutionContext>,
) -> u8 {
    let Some(context) = context else {
        return 0;
    };
    let Some((reference_id, target_id)) = theorem_pair_ids(&obligation.source_pair) else {
        return 0;
    };
    let (Some(reference_signal), Some(target_signal)) = (
        context.reference_map.get(reference_id),
        context.target_map.get(target_id),
    ) else {
        return 0;
    };
    if !reference_signal.language.eq_ignore_ascii_case("rust")
        || !target_signal.language.eq_ignore_ascii_case("rust")
    {
        return 1;
    }
    match (
        reference_signal.manifest_path.as_deref(),
        target_signal.manifest_path.as_deref(),
    ) {
        (Some(reference_manifest), Some(target_manifest)) => {
            if reference_manifest == target_manifest {
                4
            } else {
                3
            }
        }
        _ => 2,
    }
}

fn theorem_obligation_strategy_rank(obligation: &TheoremProofObligation) -> u8 {
    match theorem_obligation_assumption_value(obligation, "alignment_strategy") {
        Some("exact_path_name") => 4,
        Some("behavioral_profile_exact") => 3,
        Some("same_name_language") => 2,
        Some("signature_similarity") => 1,
        _ => 0,
    }
}

fn theorem_obligation_alignment_score(obligation: &TheoremProofObligation) -> u16 {
    theorem_obligation_assumption_value(obligation, "alignment_score")
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0)
}

fn theorem_obligation_assumption_value<'a>(
    obligation: &'a TheoremProofObligation,
    key: &str,
) -> Option<&'a str> {
    let prefix = format!("{key}=");
    obligation
        .assumptions
        .iter()
        .find_map(|assumption| assumption.strip_prefix(&prefix))
}

fn theorem_prewarm_compiler_ir_cache(
    obligations: &[TheoremProofObligation],
    context: &TheoremExecutionContext,
    policy: &TheoremPolicy,
    compiler_resources: &TheoremCompileResources,
    deadline: Instant,
) {
    if !matches!(policy.solver_mode, CliTheoremSolverMode::SmtZ3) {
        return;
    }

    let pair_obligations = theorem_pair_obligations_for_translation(
        obligations,
        Some(context),
        policy.translation_max_obligations,
    );
    if pair_obligations.is_empty() {
        return;
    }

    let mut modules = BTreeMap::<TranslationModuleKey, PathBuf>::new();
    for obligation in pair_obligations {
        if modules.len() >= THEOREM_MAX_PREWARM_MODULES {
            break;
        }
        let Some((reference_id, target_id)) = theorem_pair_ids(&obligation.source_pair) else {
            continue;
        };
        let (Some(reference_signal), Some(target_signal)) = (
            context.reference_map.get(reference_id),
            context.target_map.get(target_id),
        ) else {
            continue;
        };
        if !reference_signal.language.eq_ignore_ascii_case("rust")
            || !target_signal.language.eq_ignore_ascii_case("rust")
        {
            continue;
        }
        let (Some(reference_manifest), Some(target_manifest)) = (
            reference_signal.manifest_path.as_deref(),
            target_signal.manifest_path.as_deref(),
        ) else {
            continue;
        };

        theorem_add_prewarm_module(
            &mut modules,
            &compiler_resources.workspaces.reference_root,
            reference_manifest,
        );
        if modules.len() >= THEOREM_MAX_PREWARM_MODULES {
            break;
        }
        theorem_add_prewarm_module(
            &mut modules,
            &compiler_resources.workspaces.target_root,
            target_manifest,
        );
    }

    let mut remaining_modules = modules.len();
    for (key, workspace_root) in modules {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        let timeout =
            theorem_prewarm_timeout(deadline.saturating_duration_since(now), remaining_modules);
        let _ = theorem_cached_llvm_functions_for_manifest(
            &workspace_root,
            &compiler_resources.workspaces.shared_target_dir,
            &key.manifest_path,
            timeout,
            &compiler_resources.ir_cache,
        );
        remaining_modules = remaining_modules.saturating_sub(1);
    }
}

fn theorem_add_prewarm_module(
    modules: &mut BTreeMap<TranslationModuleKey, PathBuf>,
    workspace_root: &Path,
    manifest_path: &str,
) {
    let key = TranslationModuleKey {
        workspace_root: workspace_root.to_string_lossy().replace('\\', "/"),
        manifest_path: manifest_path.to_string(),
    };
    let _ = modules
        .entry(key)
        .or_insert_with(|| workspace_root.to_path_buf());
}

fn theorem_prewarm_timeout(remaining: Duration, remaining_modules: usize) -> Duration {
    let fair_share = remaining
        .checked_div(u32::try_from(remaining_modules.max(1)).unwrap_or(1))
        .unwrap_or(remaining);
    fair_share.max(Duration::from_millis(250)).min(remaining)
}

fn theorem_translation_workspaces(
    repo_root: &Path,
    reference_snapshot: Option<&RepositoryInventorySnapshot>,
    target_snapshot: Option<&RepositoryInventorySnapshot>,
    policy: &TheoremPolicy,
) -> TranslationWorkspaceSet {
    let target_root = target_snapshot.map_or_else(
        || repo_root.to_path_buf(),
        |snapshot| PathBuf::from(&snapshot.root),
    );
    let shared_target_dir = repo_root
        .join(".repobrain")
        .join("theorem-runs")
        .join("cargo-target");
    let _ = fs::create_dir_all(&shared_target_dir);
    let mut set = TranslationWorkspaceSet {
        reference_root: repo_root.to_path_buf(),
        target_root,
        shared_target_dir,
        reference_reason: None,
        reference_checkout: None,
    };

    let Some(reference_snapshot) = reference_snapshot else {
        set.reference_reason = Some(
            "reference snapshot metadata is unavailable; using abstract IR fallback".to_string(),
        );
        return set;
    };
    let Some(reference_revision) = reference_snapshot
        .revision
        .as_deref()
        .or_else(|| theorem_snapshot_revision_hint(&reference_snapshot.snapshot_id))
    else {
        set.reference_reason = Some(
            "reference snapshot revision is not set; compiler-emitted IR lane requires a git revision label".to_string(),
        );
        return set;
    };
    if reference_revision.eq_ignore_ascii_case("worktree") {
        set.reference_reason = Some(
            "reference snapshot points to worktree; compiler-emitted pair lane cannot materialize a distinct reference tree".to_string(),
        );
        return set;
    }

    let Some(checkout) = theorem_checkout_reference_worktree(
        repo_root,
        reference_revision,
        policy.translation_timeout_ms,
    ) else {
        set.reference_reason = Some(format!(
            "failed to materialize git worktree for reference revision `{reference_revision}`"
        ));
        return set;
    };
    set.reference_root.clone_from(&checkout.path);
    set.reference_checkout = Some(checkout);
    set
}

fn theorem_snapshot_revision_hint(snapshot_id: &str) -> Option<&str> {
    let (_, revision) = snapshot_id.rsplit_once("::")?;
    let trimmed = revision.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("worktree") {
        return None;
    }

    Some(trimmed)
}

fn theorem_checkout_reference_worktree(
    repo_root: &Path,
    reference_revision: &str,
    timeout_ms: u64,
) -> Option<GitWorktreeCheckout> {
    let mut verify = ProcessCommand::new("git");
    verify.arg("-C");
    verify.arg(repo_root);
    verify.arg("rev-parse");
    verify.arg("--verify");
    verify.arg(reference_revision);
    let verify_outcome = run_command_with_timeout(verify, Duration::from_millis(timeout_ms));
    let CommandRunOutcome::Completed(verified) = verify_outcome else {
        return None;
    };
    if !verified.status.success() {
        return None;
    }

    let seed = format!("{reference_revision}|{}", repo_root.to_string_lossy());
    let checkout_root = repo_root
        .join(".repobrain")
        .join("theorem-runs")
        .join("reference-worktrees");
    if fs::create_dir_all(&checkout_root).is_err() {
        return None;
    }
    let checkout_path = checkout_root.join(format!("{:016x}", l2_fnv1a_hash(seed.as_bytes())));
    if checkout_path.join(".git").exists() {
        return Some(GitWorktreeCheckout {
            repo_root: repo_root.to_path_buf(),
            path: checkout_path,
            cleanup_on_drop: false,
        });
    }

    let mut add = ProcessCommand::new("git");
    add.arg("-C");
    add.arg(repo_root);
    add.arg("worktree");
    add.arg("add");
    add.arg("--detach");
    add.arg("--force");
    add.arg(&checkout_path);
    add.arg(reference_revision);
    let add_outcome = run_command_with_timeout(add, Duration::from_millis(timeout_ms));
    let CommandRunOutcome::Completed(added) = add_outcome else {
        return None;
    };
    if !added.status.success() {
        return None;
    }

    Some(GitWorktreeCheckout {
        repo_root: repo_root.to_path_buf(),
        path: checkout_path,
        cleanup_on_drop: false,
    })
}

fn theorem_translation_no_pairs_receipt(policy: &TheoremPolicy) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: "alive2_translation_validation".to_string(),
        stage: EquivalenceStage::L3TheoremContract,
        status: EquivalenceEvidenceStatus::Inconclusive,
        solver: Some("alive2".to_string()),
        bounds: Some(format!(
            "alive2_per_obligation=true;checked_obligations=0;max_obligations={};timeout_ms={}",
            policy.translation_max_obligations, policy.translation_timeout_ms
        )),
        timeout_ms: Some(policy.translation_timeout_ms),
        assumptions: vec![
            "Alive2 sidecar executes per pair obligation only".to_string(),
            "no aligned pair obligations were available for translation validation".to_string(),
            "compiler-emitted LLVM IR pair extraction did not run because no pair obligations were selected".to_string(),
        ],
        witness: Some("alive2_no_pair_obligations".to_string()),
    }
}

fn theorem_translation_per_obligation_timeout(
    total_timeout_ms: u64,
    obligation_count: usize,
) -> Duration {
    Duration::from_millis(total_timeout_ms)
        .checked_div(u32::try_from(obligation_count.max(1)).unwrap_or(1))
        .unwrap_or_else(|| Duration::from_millis(total_timeout_ms))
        .max(Duration::from_millis(250))
}

fn theorem_probe_external_tool(
    binary: &str,
    repo_root: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let mut command = ProcessCommand::new(binary);
    command.arg("--version");
    command.current_dir(repo_root);
    match run_command_with_timeout(command, timeout) {
        CommandRunOutcome::Completed(_) | CommandRunOutcome::TimedOut(_) => Ok(()),
        CommandRunOutcome::SpawnFailed(error) | CommandRunOutcome::IoError(error) => {
            Err(format!("spawn_failed:{error}"))
        }
    }
}

fn theorem_translation_tool_unavailable_receipt(
    policy: &TheoremPolicy,
    alive2_binary: &str,
    reason: &str,
) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: "alive2_translation_validation".to_string(),
        stage: EquivalenceStage::L3TheoremContract,
        status: EquivalenceEvidenceStatus::Inconclusive,
        solver: Some("alive2".to_string()),
        bounds: Some(format!(
            "alive2_per_obligation=true;checked_obligations=0;max_obligations={};timeout_ms={}",
            policy.translation_max_obligations, policy.translation_timeout_ms
        )),
        timeout_ms: Some(policy.translation_timeout_ms),
        assumptions: vec![
            "Alive2 translation sidecar requires an external tool binary".to_string(),
            "configured Alive2 binary could not be started; per-obligation translation checks were skipped".to_string(),
            format!("tool probe failed for `{alive2_binary}` with `{reason}`"),
        ],
        witness: Some(format!("alive2_tool_unavailable:{alive2_binary}:{reason}")),
    }
}

fn theorem_translation_run_obligation(
    repo_root: &Path,
    context: Option<&TheoremExecutionContext>,
    compiler_resources: &TheoremCompileResources,
    obligation: &TheoremProofObligation,
    alive2_binary: &str,
    timeout: Duration,
) -> (EquivalenceEvidenceStatus, Option<String>, bool) {
    let (Some((reference_id, target_id)), Some(workspace)) = (
        theorem_pair_ids(&obligation.source_pair),
        theorem_alive2_obligation_workspace(repo_root, obligation),
    ) else {
        return (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_workspace_error:{}", obligation.id)),
            false,
        );
    };
    let Some(context) = context else {
        return (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_missing_context:{}", obligation.id)),
            false,
        );
    };
    let (Some(reference_signal), Some(target_signal)) = (
        context.reference_map.get(reference_id),
        context.target_map.get(target_id),
    ) else {
        return (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_missing_signal:{}", obligation.id)),
            false,
        );
    };
    let left_path = workspace.join("reference.ll");
    let right_path = workspace.join("target.ll");

    let compiler_lane = theorem_compiler_emitted_pair_ir(
        &compiler_resources.workspaces,
        &compiler_resources.ir_cache,
        reference_signal,
        target_signal,
        timeout,
    );
    let (left_ir, right_ir, used_compiler_ir, lane_witness) = match compiler_lane {
        Ok((left_ir, right_ir)) => (left_ir, right_ir, true, None),
        Err(error) => (
            theorem_signal_abstract_llvm_ir(reference_signal, "reference"),
            theorem_signal_abstract_llvm_ir(target_signal, "target"),
            false,
            Some(format!(
                "alive2_compiler_ir_fallback:{}:{}",
                obligation.id, error
            )),
        ),
    };
    if fs::write(&left_path, left_ir).is_err() || fs::write(&right_path, right_ir).is_err() {
        return (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_ir_write_error:{}", obligation.id)),
            used_compiler_ir,
        );
    }

    let run = run_alive2_obligation(alive2_binary, &left_path, &right_path, repo_root, timeout);
    let (status, witness) = theorem_translation_outcome(run, obligation);
    let witness = witness.or(lane_witness);
    (status, witness, used_compiler_ir)
}

fn theorem_translation_record_outcome(
    stats: &mut TranslationRunStats,
    outcome: EquivalenceEvidenceStatus,
    candidate_witness: Option<String>,
) {
    if stats.witness.is_none() {
        stats.witness = candidate_witness;
    }
    match outcome {
        EquivalenceEvidenceStatus::NoDifferenceObserved => {
            stats.proved += 1;
        }
        EquivalenceEvidenceStatus::ObservedDifference => {
            stats.refuted += 1;
        }
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing
        | EquivalenceEvidenceStatus::Inconclusive => {
            stats.inconclusive += 1;
        }
    }
}

fn theorem_translation_receipt_from_stats(
    policy: &TheoremPolicy,
    stats: TranslationRunStats,
    reference_reason: Option<&String>,
    checked_obligations: usize,
) -> EquivalenceEvidenceReceipt {
    let mut overall_status = if stats.refuted > 0 {
        EquivalenceEvidenceStatus::ObservedDifference
    } else if stats.proved > 0 && stats.inconclusive == 0 {
        EquivalenceEvidenceStatus::NoDifferenceObserved
    } else {
        EquivalenceEvidenceStatus::Inconclusive
    };
    let mut witness = stats.witness;
    if matches!(
        policy.translation_ir_mode,
        CliTranslationIrMode::RequireCompilerIr
    ) && checked_obligations > 0
        && stats.abstract_fallback_runs > 0
    {
        if !matches!(
            overall_status,
            EquivalenceEvidenceStatus::ObservedDifference
        ) {
            overall_status = EquivalenceEvidenceStatus::Inconclusive;
        }
        if witness.is_none() {
            witness = Some(format!(
                "alive2_compiler_ir_required_fallback_runs:{}",
                stats.abstract_fallback_runs
            ));
        }
    }

    EquivalenceEvidenceReceipt {
        backend: "alive2_translation_validation".to_string(),
        stage: EquivalenceStage::L3TheoremContract,
        status: overall_status,
        solver: Some("alive2".to_string()),
        bounds: Some(format!(
            "alive2_per_obligation=true;checked_obligations={};proved={};refuted={};inconclusive={};compiler_ir_runs={};abstract_fallback_runs={};max_obligations={};timeout_ms={}",
            checked_obligations,
            stats.proved,
            stats.refuted,
            stats.inconclusive,
            stats.compiler_ir_runs,
            stats.abstract_fallback_runs,
            policy.translation_max_obligations,
            policy.translation_timeout_ms
        )),
        timeout_ms: Some(policy.translation_timeout_ms),
        assumptions: theorem_translation_assumptions(
            checked_obligations,
            stats.compiler_ir_runs,
            stats.abstract_fallback_runs,
            reference_reason,
            matches!(
                policy.translation_ir_mode,
                CliTranslationIrMode::RequireCompilerIr
            ),
        ),
        witness,
    }
}

fn theorem_translation_assumptions(
    checked_obligations: usize,
    compiler_ir_runs: usize,
    abstract_fallback_runs: usize,
    reference_reason: Option<&String>,
    require_compiler_ir: bool,
) -> Vec<String> {
    let mut assumptions = Vec::new();
    assumptions.push(format!(
        "Alive2 sidecar executed per-obligation translation checks over {checked_obligations} obligation(s)"
    ));
    assumptions.push(format!(
        "compiler-emitted LLVM IR pair checks succeeded for {compiler_ir_runs} obligation(s); abstract IR fallback was used for {abstract_fallback_runs} obligation(s)"
    ));
    assumptions.push(
        "translation-validation lane is bounded and may be inconclusive when tooling is missing, revisions are unavailable, or IR extraction fails".to_string(),
    );
    if require_compiler_ir {
        assumptions.push(
            "translation policy requires compiler-emitted LLVM IR for no-difference claims; abstract fallback cannot finalize equivalence".to_string(),
        );
    }
    if let Some(reason) = reference_reason {
        assumptions.push(format!("reference workspace note: {reason}"));
    }

    assumptions
}

fn theorem_translation_outcome(
    run: CommandRunOutcome,
    obligation: &TheoremProofObligation,
) -> (EquivalenceEvidenceStatus, Option<String>) {
    match run {
        CommandRunOutcome::Completed(output) => {
            let stdout = summarize_command_output(&output.stdout);
            let stderr = summarize_command_output(&output.stderr);
            let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
            if output.status.success() && !combined.contains("not equivalent") {
                (
                    EquivalenceEvidenceStatus::NoDifferenceObserved,
                    first_non_empty_line(&stdout)
                        .or_else(|| Some(format!("alive2_equivalent:{}", obligation.id))),
                )
            } else if combined.contains("not equivalent")
                || combined.contains("counterexample")
                || combined.contains("mismatch")
            {
                (
                    EquivalenceEvidenceStatus::ObservedDifference,
                    Some(format!("alive2_counterexample:{}", obligation.id)),
                )
            } else {
                (
                    EquivalenceEvidenceStatus::Inconclusive,
                    first_non_empty_line(&stderr).map(|line| {
                        format!(
                            "alive2_non_zero_exit:{}:{}:{line}",
                            obligation.id,
                            output.status.code().unwrap_or(-1)
                        )
                    }),
                )
            }
        }
        CommandRunOutcome::TimedOut(_) => (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_timeout:{}", obligation.id)),
        ),
        CommandRunOutcome::SpawnFailed(error) | CommandRunOutcome::IoError(error) => (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!("alive2_runtime_error:{}:{error}", obligation.id)),
        ),
    }
}

fn run_alive2_obligation(
    alive2_binary: &str,
    left_path: &Path,
    right_path: &Path,
    repo_root: &Path,
    timeout: Duration,
) -> CommandRunOutcome {
    let mut command = ProcessCommand::new(alive2_binary);
    command.arg("-allow-incomplete-ir");
    command.arg(left_path);
    command.arg(right_path);
    command.current_dir(repo_root);
    run_command_with_timeout(command, timeout)
}

fn theorem_alive2_obligation_workspace(
    repo_root: &Path,
    obligation: &TheoremProofObligation,
) -> Option<PathBuf> {
    let workspace = repo_root
        .join(".repobrain")
        .join("theorem-runs")
        .join("alive2")
        .join(&obligation.id);
    if fs::create_dir_all(&workspace).is_err() {
        return None;
    }

    Some(workspace)
}

fn theorem_signal_abstract_llvm_ir(signal: &L2FunctionSignal, lane: &str) -> String {
    let behavior_hash = l2_fnv1a_hash(signal.behavior_signature.as_bytes());
    let behavior_constant = i64::from_ne_bytes(behavior_hash.to_ne_bytes());
    format!(
        "; RepoBrain Alive2 abstract lane={lane}\n; signal={}\n; path={}\ndefine i64 @repobrain_obligation() {{\nentry:\n  ret i64 {}\n}}\n",
        signal.id, signal.relative_path, behavior_constant
    )
}

fn theorem_compiler_emitted_pair_ir(
    workspaces: &TranslationWorkspaceSet,
    ir_cache: &Mutex<TranslationIrCache>,
    reference_signal: &L2FunctionSignal,
    target_signal: &L2FunctionSignal,
    timeout: Duration,
) -> Result<(String, String), String> {
    if !reference_signal.language.eq_ignore_ascii_case("rust")
        || !target_signal.language.eq_ignore_ascii_case("rust")
    {
        return Err("non_rust_pair".to_string());
    }
    let Some(reference_manifest) = reference_signal.manifest_path.as_deref() else {
        return Err(format!(
            "missing_reference_manifest:{}",
            reference_signal.relative_path
        ));
    };
    let Some(target_manifest) = target_signal.manifest_path.as_deref() else {
        return Err(format!(
            "missing_target_manifest:{}",
            target_signal.relative_path
        ));
    };

    let left_ir = theorem_compiler_emitted_signal_ir(
        &workspaces.reference_root,
        &workspaces.shared_target_dir,
        reference_manifest,
        reference_signal,
        timeout,
        ir_cache,
    )?;
    let right_ir = theorem_compiler_emitted_signal_ir(
        &workspaces.target_root,
        &workspaces.shared_target_dir,
        target_manifest,
        target_signal,
        timeout,
        ir_cache,
    )?;

    Ok((left_ir, right_ir))
}

fn theorem_compiler_emitted_signal_ir(
    workspace_root: &Path,
    shared_target_dir: &Path,
    manifest_path: &str,
    signal: &L2FunctionSignal,
    timeout: Duration,
    ir_cache: &Mutex<TranslationIrCache>,
) -> Result<String, String> {
    let functions = theorem_cached_llvm_functions_for_manifest(
        workspace_root,
        shared_target_dir,
        manifest_path,
        timeout,
        ir_cache,
    )?;
    let Some(best) = theorem_best_llvm_function_for_signal(&functions, signal, workspace_root)
    else {
        return Err(format!(
            "llvm_function_not_found:{}@{}:{}",
            signal.name, signal.relative_path, signal.line_number
        ));
    };

    Ok(best.ir.clone())
}

fn theorem_cached_llvm_functions_for_manifest(
    workspace_root: &Path,
    shared_target_dir: &Path,
    manifest_path: &str,
    timeout: Duration,
    ir_cache: &Mutex<TranslationIrCache>,
) -> Result<Vec<ExtractedLlvmFunction>, String> {
    let key = TranslationModuleKey {
        workspace_root: workspace_root.to_string_lossy().replace('\\', "/"),
        manifest_path: manifest_path.to_string(),
    };
    let mut cache = ir_cache
        .lock()
        .map_err(|_| "llvm_cache_poisoned".to_string())?;
    if !cache.modules.contains_key(&key) {
        let compiled = theorem_compile_manifest_llvm_functions(
            workspace_root,
            shared_target_dir,
            manifest_path,
            timeout,
        );
        cache.modules.insert(key.clone(), compiled);
    }

    cache
        .modules
        .get(&key)
        .cloned()
        .unwrap_or_else(|| Err("llvm_cache_lookup_failed".to_string()))
}

fn theorem_compile_manifest_llvm_functions(
    workspace_root: &Path,
    shared_target_dir: &Path,
    manifest_path: &str,
    timeout: Duration,
) -> Result<Vec<ExtractedLlvmFunction>, String> {
    let mut command = ProcessCommand::new("cargo");
    command.arg("rustc");
    command.arg("--manifest-path");
    command.arg(manifest_path);
    command.arg("--target-dir");
    command.arg(shared_target_dir);
    command.arg("--message-format=json");
    command.arg("--");
    command.arg("--emit=llvm-ir");
    command.arg("-Cdebuginfo=1");
    command.current_dir(workspace_root);

    let run = run_command_with_timeout(command, timeout);
    let output = match run {
        CommandRunOutcome::Completed(output) => output,
        CommandRunOutcome::TimedOut(output) => {
            let details = output.as_ref().map_or_else(
                || "timeout".to_string(),
                |captured| {
                    first_non_empty_line(&summarize_command_output(&captured.stderr))
                        .unwrap_or_else(|| "timeout".to_string())
                },
            );
            return Err(format!("cargo_rustc_timeout:{manifest_path}:{details}"));
        }
        CommandRunOutcome::SpawnFailed(error) | CommandRunOutcome::IoError(error) => {
            return Err(format!("cargo_rustc_spawn_error:{manifest_path}:{error}"));
        }
    };

    if !output.status.success() {
        let stderr = summarize_command_output(&output.stderr);
        let stdout = summarize_command_output(&output.stdout);
        let line = first_non_empty_line(&stderr)
            .or_else(|| first_non_empty_line(&stdout))
            .unwrap_or_else(|| "cargo_rustc_failed".to_string());
        return Err(format!(
            "cargo_rustc_non_zero_exit:{}:{}:{}",
            manifest_path,
            output.status.code().unwrap_or(-1),
            line
        ));
    }

    let artifact_paths = theorem_collect_llvm_artifact_paths(
        &output.stdout,
        shared_target_dir,
        workspace_root,
        manifest_path,
    );
    if artifact_paths.is_empty() {
        return Err(format!("no_llvm_artifacts:{manifest_path}"));
    }

    let mut functions = Vec::<ExtractedLlvmFunction>::new();
    for artifact in artifact_paths {
        let Ok(contents) = fs::read_to_string(&artifact) else {
            continue;
        };
        functions.extend(theorem_parse_llvm_module_functions(&contents));
    }
    if functions.is_empty() {
        return Err(format!("no_llvm_functions_parsed:{manifest_path}"));
    }

    Ok(functions)
}

fn theorem_parse_llvm_module_functions(module: &str) -> Vec<ExtractedLlvmFunction> {
    let mut file_metadata = BTreeMap::<usize, String>::new();
    for line in module.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("!DIFile(") {
            continue;
        }
        let Some(id) = theorem_metadata_id(trimmed) else {
            continue;
        };
        let Some(filename) = theorem_extract_quoted_value(trimmed, "filename: \"") else {
            continue;
        };
        let directory = theorem_extract_quoted_value(trimmed, "directory: \"").unwrap_or_default();
        let full_path = if filename.contains(':')
            || filename.starts_with('/')
            || filename.starts_with('\\')
            || directory.is_empty()
        {
            filename
        } else {
            format!("{directory}/{filename}")
        };
        file_metadata.insert(id, full_path.replace('\\', "/"));
    }

    let mut subprogram_metadata = BTreeMap::<usize, (String, Option<String>, Option<u32>)>::new();
    for line in module.lines() {
        let trimmed = line.trim();
        if !trimmed.contains("!DISubprogram(") {
            continue;
        }
        let Some(id) = theorem_metadata_id(trimmed) else {
            continue;
        };
        let Some(name) = theorem_extract_quoted_value(trimmed, "name: \"") else {
            continue;
        };
        let file_id = theorem_extract_metadata_ref(trimmed, "file: !");
        let line_number = theorem_extract_numeric_field(trimmed, "line: ");
        let source_path = file_id.and_then(|value| file_metadata.get(&value).cloned());
        subprogram_metadata.insert(id, (name, source_path, line_number));
    }

    let lines = module.lines().collect::<Vec<_>>();
    let mut functions = Vec::<ExtractedLlvmFunction>::new();
    let mut index = 0_usize;
    while index < lines.len() {
        let define_line = lines[index].trim_start();
        if !define_line.starts_with("define ") {
            index += 1;
            continue;
        }

        let mut block = String::new();
        let mut cursor = index;
        let mut opened = false;
        let mut balance = 0_i64;
        while cursor < lines.len() {
            let line = lines[cursor];
            block.push_str(line);
            block.push('\n');
            for byte in line.as_bytes() {
                if *byte == b'{' {
                    opened = true;
                    balance += 1;
                } else if *byte == b'}' && opened {
                    balance -= 1;
                }
            }
            if opened && balance <= 0 {
                break;
            }
            cursor += 1;
        }

        let symbol_name = theorem_extract_function_symbol_name(define_line).unwrap_or_default();
        let dbg_id = theorem_extract_metadata_ref(define_line, "!dbg !");
        let (subprogram_name, source_path, line_number) = dbg_id
            .and_then(|value| subprogram_metadata.get(&value))
            .map_or((None, None, None), |entry| {
                (Some(entry.0.clone()), entry.1.clone(), entry.2)
            });

        functions.push(ExtractedLlvmFunction {
            symbol_name,
            subprogram_name,
            source_path,
            line_number,
            ir: block,
        });

        index = cursor.saturating_add(1);
    }

    functions
}

fn theorem_metadata_id(line: &str) -> Option<usize> {
    let payload = line.trim_start().strip_prefix('!')?;
    let digits = payload
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }

    digits.parse::<usize>().ok()
}

fn theorem_extract_quoted_value(line: &str, marker: &str) -> Option<String> {
    let start = line.find(marker)? + marker.len();
    let tail = &line[start..];
    let end = tail.find('"')?;
    Some(tail[..end].to_string())
}

fn theorem_extract_metadata_ref(line: &str, marker: &str) -> Option<usize> {
    let start = line.find(marker)? + marker.len();
    let digits = line[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }

    digits.parse::<usize>().ok()
}

fn theorem_extract_numeric_field(line: &str, marker: &str) -> Option<u32> {
    let start = line.find(marker)? + marker.len();
    let digits = line[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }

    digits.parse::<u32>().ok()
}

fn theorem_extract_function_symbol_name(define_line: &str) -> Option<String> {
    let at_index = define_line.find('@')?;
    let payload = &define_line[at_index + 1..];
    if let Some(quoted) = payload.strip_prefix('"') {
        let end = quoted.find('"')?;
        return Some(quoted[..end].to_string());
    }

    let end = payload.find('(')?;
    Some(payload[..end].trim().to_string())
}

fn theorem_best_llvm_function_for_signal<'a>(
    functions: &'a [ExtractedLlvmFunction],
    signal: &L2FunctionSignal,
    workspace_root: &Path,
) -> Option<&'a ExtractedLlvmFunction> {
    let signal_name = signal.name.as_str();
    let signal_path = signal.relative_path.replace('\\', "/").to_ascii_lowercase();
    let mut best = None;
    let mut best_score = i32::MIN;

    for function in functions {
        let mut score = 0_i32;
        let mut eligible = false;
        if function.subprogram_name.as_deref() == Some(signal_name) {
            score += 120;
            eligible = true;
        } else if function.symbol_name.contains(signal_name) {
            score += 40;
            eligible = true;
        }

        if let Some(path) = &function.source_path {
            let normalized = path.replace('\\', "/").to_ascii_lowercase();
            if normalized.ends_with(&signal_path) {
                score += 80;
                eligible = true;
            } else if let Ok(relative) = Path::new(path).strip_prefix(workspace_root) {
                let relative = relative
                    .to_string_lossy()
                    .replace('\\', "/")
                    .to_ascii_lowercase();
                if relative == signal_path {
                    score += 80;
                    eligible = true;
                }
            }
        }

        if let Some(line_number) = function.line_number {
            let distance = line_number.abs_diff(signal.line_number);
            if distance <= 2 {
                score += 25;
            } else if distance <= 8 {
                score += 16;
            } else if distance <= 20 {
                score += 8;
            }
        }

        if !eligible {
            continue;
        }
        if score > best_score {
            best_score = score;
            best = Some(function);
        }
    }

    best
}

fn theorem_run_status_from_certificates(
    certificates: &[TheoremProofCertificate],
) -> TheoremRunStatus {
    if certificates
        .iter()
        .any(|certificate| matches!(certificate.status, TheoremObligationStatus::Refuted))
    {
        return TheoremRunStatus::RefutedUnderContract;
    }
    if !certificates.is_empty()
        && certificates
            .iter()
            .all(|certificate| matches!(certificate.status, TheoremObligationStatus::Proved))
    {
        return TheoremRunStatus::ProvedUnderContract;
    }

    TheoremRunStatus::InconclusiveUnderContract
}

fn theorem_run_status_with_translation(
    base_run_status: TheoremRunStatus,
    extra_receipts: &[EquivalenceEvidenceReceipt],
    policy: &TheoremPolicy,
) -> TheoremRunStatus {
    if !policy.translation_validation
        || !matches!(
            policy.translation_run_status_mode,
            CliTranslationRunStatusMode::Override
        )
    {
        return base_run_status;
    }
    let Some(translation_receipt) = extra_receipts
        .iter()
        .find(|receipt| receipt.backend == "alive2_translation_validation")
    else {
        return base_run_status;
    };

    match translation_receipt.status {
        EquivalenceEvidenceStatus::ObservedDifference => TheoremRunStatus::RefutedUnderContract,
        EquivalenceEvidenceStatus::NoDifferenceObserved => base_run_status,
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => {
            if matches!(base_run_status, TheoremRunStatus::RefutedUnderContract) {
                base_run_status
            } else {
                TheoremRunStatus::InconclusiveUnderContract
            }
        }
        EquivalenceEvidenceStatus::Inconclusive => {
            if matches!(base_run_status, TheoremRunStatus::RefutedUnderContract) {
                base_run_status
            } else if matches!(
                policy.translation_ir_mode,
                CliTranslationIrMode::RequireCompilerIr
            ) {
                TheoremRunStatus::InconclusiveUnderContract
            } else {
                base_run_status
            }
        }
    }
}

pub(super) fn theorem_stage_missing_contract_receipt(
    policy: &TheoremPolicy,
) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: "theorem_obligation_compiler".to_string(),
        stage: EquivalenceStage::L3TheoremContract,
        status: EquivalenceEvidenceStatus::Inconclusive,
        solver: None,
        bounds: Some(format!(
            "max_obligations<={};candidate_functions<={};solver_mode={};solver_timeout_ms={}",
            policy.max_obligations,
            policy.max_candidate_functions,
            theorem_solver_label(policy),
            policy.solver_timeout_ms
        )),
        timeout_ms: Some(policy.timeout_ms),
        assumptions: vec![
            "theorem stage requires an explicit theorem contract file".to_string(),
            "no obligations were compiled".to_string(),
        ],
        witness: Some("missing_theorem_contract".to_string()),
    }
}

pub(super) fn theorem_stage_receipt(
    artifacts: &TheoremStageArtifacts,
    policy: &TheoremPolicy,
) -> EquivalenceEvidenceReceipt {
    let run_status = artifacts
        .run_status
        .unwrap_or(TheoremRunStatus::InconclusiveUnderContract);
    let status = match run_status {
        TheoremRunStatus::ProvedUnderContract => EquivalenceEvidenceStatus::NoDifferenceObserved,
        TheoremRunStatus::RefutedUnderContract => EquivalenceEvidenceStatus::ObservedDifference,
        TheoremRunStatus::InconclusiveUnderContract => EquivalenceEvidenceStatus::Inconclusive,
    };
    let witness = artifacts
        .certificates
        .iter()
        .find_map(|certificate| certificate.witness.clone());
    let solver = artifacts
        .certificates
        .iter()
        .find_map(|certificate| certificate.solver.clone());
    let proved = artifacts
        .certificates
        .iter()
        .filter(|certificate| matches!(certificate.status, TheoremObligationStatus::Proved))
        .count();
    let refuted = artifacts
        .certificates
        .iter()
        .filter(|certificate| matches!(certificate.status, TheoremObligationStatus::Refuted))
        .count();
    let inconclusive = artifacts
        .certificates
        .iter()
        .filter(|certificate| matches!(certificate.status, TheoremObligationStatus::Inconclusive))
        .count();
    let translation_note = artifacts
        .extra_receipts
        .iter()
        .find(|receipt| receipt.backend == "alive2_translation_validation")
        .map(|receipt| {
            format!(
                "Alive2 translation sidecar status={} under configured bounds{}",
                theorem_equivalence_evidence_status_label(receipt.status),
                receipt
                    .bounds
                    .as_deref()
                    .map(|bounds| format!(" ({bounds})"))
                    .unwrap_or_default()
            )
        });
    let mut assumptions = vec![
        "L3 executes bounded obligation-level theorem checks with deterministic scheduling and a compiler-IR-first LLVM SSA/IVL backend"
            .to_string(),
        "proved/refuted outcomes are bounded by contract assumptions, supported LLVM semantics, explicit memory/pointer/call summaries, and fallback lanes when compiler IR is unavailable"
            .to_string(),
    ];
    if let Some(note) = translation_note {
        assumptions.push(note);
    }

    EquivalenceEvidenceReceipt {
        backend: "theorem_obligation_compiler".to_string(),
        stage: EquivalenceStage::L3TheoremContract,
        status,
        solver,
        bounds: Some(format!(
            "max_obligations<={};candidate_functions<={};compiled_obligations={};proved={};refuted={};inconclusive={};stability_runs={};flaky_retries={};solver_mode={};solver_timeout_ms={}",
            policy.max_obligations,
            policy.max_candidate_functions,
            artifacts.obligations.len(),
            proved,
            refuted,
            inconclusive,
            policy.stability_runs,
            policy.flaky_retries,
            theorem_solver_label(policy),
            policy.solver_timeout_ms
        )),
        timeout_ms: Some(policy.timeout_ms),
        assumptions,
        witness,
    }
}

pub(super) fn theorem_stage_summary(
    artifacts: &TheoremStageArtifacts,
    delta: &SemanticDelta,
    reference_snapshot_id: &str,
    target_snapshot_id: &str,
) -> String {
    let structural = semantic_diff_summary(delta, reference_snapshot_id, target_snapshot_id);
    let run_status = artifacts
        .run_status
        .unwrap_or(TheoremRunStatus::InconclusiveUnderContract);
    let translation_suffix = artifacts
        .extra_receipts
        .iter()
        .find(|receipt| receipt.backend == "alive2_translation_validation")
        .map(|receipt| {
            format!(
                " Translation sidecar status: {}.",
                theorem_equivalence_evidence_status_label(receipt.status)
            )
        })
        .unwrap_or_default();

    match run_status {
        TheoremRunStatus::ProvedUnderContract => format!(
            "theorem stage reports proved_under_contract for {} obligation(s) after LLVM SSA/IVL semantic-state and effect-summary checking. Structural baseline: {structural}{translation_suffix}",
            artifacts.obligations.len(),
        ),
        TheoremRunStatus::RefutedUnderContract => format!(
            "theorem stage reports refuted_under_contract for at least one obligation after LLVM SSA/IVL semantic-state and effect-summary checking. Structural baseline: {structural}{translation_suffix}"
        ),
        TheoremRunStatus::InconclusiveUnderContract => format!(
            "theorem stage executed {} obligation(s) under explicit contract and remained partially inconclusive under configured bounds after trying the LLVM SSA/IVL backend with bounded memory/pointer/call summaries and fallbacks. Structural baseline: {structural}{translation_suffix}",
            artifacts.obligations.len(),
        ),
    }
}

pub(super) fn theorem_run_status_label(status: TheoremRunStatus) -> &'static str {
    match status {
        TheoremRunStatus::ProvedUnderContract => "proved_under_contract",
        TheoremRunStatus::RefutedUnderContract => "refuted_under_contract",
        TheoremRunStatus::InconclusiveUnderContract => "inconclusive_under_contract",
    }
}

pub(super) fn theorem_equivalence_evidence_status_label(
    status: EquivalenceEvidenceStatus,
) -> &'static str {
    match status {
        EquivalenceEvidenceStatus::ObservedDifference => "observed_difference",
        EquivalenceEvidenceStatus::NoDifferenceObserved => "no_difference_observed",
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => "reference_snapshot_missing",
        EquivalenceEvidenceStatus::Inconclusive => "inconclusive",
    }
}

#[cfg(test)]
mod tests;
