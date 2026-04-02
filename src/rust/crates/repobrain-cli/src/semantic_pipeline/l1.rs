use super::{
    BTreeSet, CliL1KaniMode, CommandRunOutcome, Duration, EquivalenceEvidenceReceipt,
    EquivalenceEvidenceStatus, EquivalenceStage, Instant, L1ObligationExecution, L1ProofObligation,
    L1RustBoundedPolicy, L1RustBoundedScope, Output, Path, PathBuf, ProcessCommand,
    RepositoryInventorySnapshot, SemanticDelta, Stdio, SymbolKind, fs, semantic_delta_witness,
    semantic_diff_summary,
};
use std::sync::mpsc::{RecvTimeoutError, sync_channel};
use std::thread;

pub(super) fn l1_rust_bounded_receipt(
    repo_root: &Path,
    target_snapshot: &RepositoryInventorySnapshot,
    delta: &SemanticDelta,
    policy: L1RustBoundedPolicy,
) -> EquivalenceEvidenceReceipt {
    let scope = l1_rust_bounded_scope(
        target_snapshot,
        delta,
        policy.max_rust_files,
        policy.max_functions,
    );
    let mut assumptions = l1_base_assumptions();
    let mut bounds = l1_bounds(policy, 0);

    if scope.rust_files.is_empty() {
        assumptions
            .push("no changed Rust files were present in the structural delta scope".to_string());
        return l1_receipt(
            EquivalenceEvidenceStatus::Inconclusive,
            bounds,
            policy.timeout_ms,
            assumptions,
            semantic_delta_witness(delta),
        );
    }

    assumptions.push(format!(
        "candidate scope selected {} rust file(s) and {} function symbol(s)",
        scope.rust_files.len(),
        scope.function_symbols.len()
    ));

    if matches!(policy.kani_mode, CliL1KaniMode::Probe) {
        let witness = l1_scope_witness(&scope);
        let run = run_kani_with_timeout(repo_root, policy);
        return l1_receipt_from_run(run, policy, bounds, assumptions, witness);
    }

    let obligations = l1_collect_proof_obligations(repo_root, &scope, policy.max_functions);
    bounds = l1_bounds(policy, obligations.len());
    assumptions.push(format!(
        "targeted obligation selection discovered {} Kani harness obligation(s)",
        obligations.len()
    ));
    if obligations.is_empty() {
        assumptions.push(
            "no #[kani::proof] harnesses were discovered in changed Rust scope; targeted check could not execute obligations"
                .to_string(),
        );
        return l1_receipt(
            EquivalenceEvidenceStatus::Inconclusive,
            bounds,
            policy.timeout_ms,
            assumptions,
            l1_scope_witness(&scope),
        );
    }

    let execution = run_l1_targeted_obligations(repo_root, &obligations, policy);
    assumptions.push(format!(
        "targeted obligation run results: no_difference={}, observed_difference={}, inconclusive={}",
        execution.no_difference_observed, execution.observed_difference, execution.inconclusive
    ));
    if execution.timed_out {
        assumptions
            .push("targeted obligation run ended due to timeout budget exhaustion".to_string());
    }
    let status = l1_status_from_execution(&execution);
    l1_receipt(
        status,
        bounds,
        policy.timeout_ms,
        assumptions,
        execution
            .witness
            .or_else(|| l1_scope_witness(&scope))
            .or_else(|| semantic_delta_witness(delta)),
    )
}

fn l1_base_assumptions() -> Vec<String> {
    vec![
        "L1 bounded formal stage uses Kani as a Rust-focused backend and remains bounded by explicit scope limits".to_string(),
        "check mode executes selected #[kani::proof] harness obligations instead of workspace-wide blanket invocation".to_string(),
        "absence of a counterexample under bounds is not an unrestricted semantic equivalence proof".to_string(),
    ]
}

fn l1_bounds(policy: L1RustBoundedPolicy, obligation_count: usize) -> String {
    format!(
        "rust_files<={};functions<={};obligations<={};mode={}",
        policy.max_rust_files,
        policy.max_functions,
        obligation_count,
        l1_kani_mode_label(policy.kani_mode)
    )
}

fn l1_receipt_from_run(
    run: CommandRunOutcome,
    policy: L1RustBoundedPolicy,
    bounds: String,
    assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    match run {
        CommandRunOutcome::Completed(output) => {
            l1_receipt_from_completed(&output, policy, bounds, assumptions, witness)
        }
        CommandRunOutcome::TimedOut(output) => {
            l1_receipt_from_timeout(output, policy, bounds, assumptions, witness)
        }
        CommandRunOutcome::SpawnFailed(error) | CommandRunOutcome::IoError(error) => {
            l1_receipt_from_runtime_error(&error, policy, bounds, assumptions, witness)
        }
    }
}

fn l1_receipt_from_completed(
    output: &Output,
    policy: L1RustBoundedPolicy,
    bounds: String,
    mut assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    let stdout = summarize_command_output(&output.stdout);
    let stderr = summarize_command_output(&output.stderr);
    if output.status.success() {
        let status = if matches!(policy.kani_mode, CliL1KaniMode::Check) {
            EquivalenceEvidenceStatus::NoDifferenceObserved
        } else {
            EquivalenceEvidenceStatus::Inconclusive
        };
        assumptions.push(success_assumption_for_mode(policy.kani_mode));
        return l1_receipt(
            status,
            bounds,
            policy.timeout_ms,
            assumptions,
            witness.or_else(|| first_non_empty_line(&stdout)),
        );
    }

    assumptions.push(
        "non-zero Kani exit is interpreted as a counterexample only when explicit failure markers are present"
            .to_string(),
    );
    let status = kani_failure_status(&stdout, &stderr);
    let failure_witness = first_non_empty_line(&stderr)
        .or_else(|| first_non_empty_line(&stdout))
        .map(|line| format!("kani_exit:{}:{line}", output.status.code().unwrap_or(-1)));
    l1_receipt(
        status,
        bounds,
        policy.timeout_ms,
        assumptions,
        witness.or(failure_witness),
    )
}

fn success_assumption_for_mode(mode: CliL1KaniMode) -> String {
    match mode {
        CliL1KaniMode::Probe => {
            "probe mode checks Kani toolchain availability only; no proof obligations are executed"
                .to_string()
        }
        CliL1KaniMode::Check => {
            "check mode completed within bounds and timeout without a reported counterexample"
                .to_string()
        }
    }
}

fn l1_receipt_from_timeout(
    output: Option<Output>,
    policy: L1RustBoundedPolicy,
    bounds: String,
    mut assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    assumptions.push(format!(
        "command exceeded timeout policy ({}ms) and was terminated",
        policy.timeout_ms
    ));
    let timeout_witness = output.and_then(|captured| {
        let stderr = summarize_command_output(&captured.stderr);
        let stdout = summarize_command_output(&captured.stdout);
        first_non_empty_line(&stderr).or_else(|| first_non_empty_line(&stdout))
    });
    l1_receipt(
        EquivalenceEvidenceStatus::Inconclusive,
        bounds,
        policy.timeout_ms,
        assumptions,
        witness.or(timeout_witness),
    )
}

fn l1_receipt_from_runtime_error(
    error: &str,
    policy: L1RustBoundedPolicy,
    bounds: String,
    mut assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    assumptions.push("Kani execution could not be completed in this environment".to_string());
    l1_receipt(
        EquivalenceEvidenceStatus::Inconclusive,
        bounds,
        policy.timeout_ms,
        assumptions,
        witness.or(Some(format!("kani_runtime_error:{error}"))),
    )
}

fn l1_receipt(
    status: EquivalenceEvidenceStatus,
    bounds: String,
    timeout_ms: u64,
    assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: "kani_rust_bounded".to_string(),
        stage: EquivalenceStage::L1BoundedFormal,
        status,
        solver: Some("cbmc".to_string()),
        bounds: Some(bounds),
        timeout_ms: Some(timeout_ms),
        assumptions,
        witness,
    }
}

pub(crate) fn l1_rust_bounded_scope(
    target_snapshot: &RepositoryInventorySnapshot,
    delta: &SemanticDelta,
    max_rust_files: usize,
    max_functions: usize,
) -> L1RustBoundedScope {
    let rust_files = collect_rust_scope_paths(delta, max_rust_files);
    let rust_scope = rust_files.iter().cloned().collect::<BTreeSet<_>>();
    let function_symbols = target_snapshot
        .symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Function))
        .filter(|symbol| rust_scope.contains(&symbol.relative_path))
        .take(max_functions)
        .map(|symbol| {
            format!(
                "{}@{}:{}",
                symbol.name, symbol.relative_path, symbol.line_number
            )
        })
        .collect::<Vec<_>>();

    L1RustBoundedScope {
        rust_files,
        function_symbols,
    }
}

fn collect_rust_scope_paths(delta: &SemanticDelta, max_rust_files: usize) -> Vec<String> {
    let mut rust_files = BTreeSet::new();
    for candidates in [
        &delta.changed_scope,
        &delta.modified_files,
        &delta.added_files,
        &delta.removed_files,
    ] {
        for path in candidates {
            if is_rust_source_path(path) {
                let _ = rust_files.insert(path.clone());
            }
        }
        if !rust_files.is_empty() {
            break;
        }
    }

    rust_files.into_iter().take(max_rust_files).collect()
}

pub(crate) fn is_rust_source_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
}

fn l1_scope_witness(scope: &L1RustBoundedScope) -> Option<String> {
    if let Some(symbol) = scope.function_symbols.first() {
        return Some(format!("l1_function:{symbol}"));
    }

    scope
        .rust_files
        .first()
        .map(|path| format!("l1_rust_file:{path}"))
}

fn run_l1_targeted_obligations(
    repo_root: &Path,
    obligations: &[L1ProofObligation],
    policy: L1RustBoundedPolicy,
) -> L1ObligationExecution {
    let mut execution = L1ObligationExecution {
        observed_difference: 0,
        no_difference_observed: 0,
        inconclusive: 0,
        timed_out: false,
        witness: None,
    };
    let deadline = Instant::now() + Duration::from_millis(policy.timeout_ms);
    let per_obligation_timeout_ms =
        l1_per_obligation_timeout_ms(policy.timeout_ms, obligations.len());

    for obligation in obligations {
        let now = Instant::now();
        if now >= deadline {
            execution.timed_out = true;
            execution.inconclusive += 1;
            execution
                .witness
                .get_or_insert_with(|| format!("l1_timeout:{}", l1_obligation_label(obligation)));
            break;
        }

        let remaining = deadline.saturating_duration_since(now);
        let timeout = remaining.min(Duration::from_millis(per_obligation_timeout_ms));
        let run = run_kani_obligation_with_timeout(repo_root, obligation, timeout);
        let (status, witness, timed_out) = l1_obligation_outcome(run, obligation);
        if timed_out {
            execution.timed_out = true;
        }
        if execution.witness.is_none() {
            execution.witness = witness;
        }
        match status {
            EquivalenceEvidenceStatus::ObservedDifference => {
                execution.observed_difference += 1;
                break;
            }
            EquivalenceEvidenceStatus::NoDifferenceObserved => {
                execution.no_difference_observed += 1;
            }
            EquivalenceEvidenceStatus::ReferenceSnapshotMissing
            | EquivalenceEvidenceStatus::Inconclusive => {
                execution.inconclusive += 1;
            }
        }
    }

    execution
}

fn l1_per_obligation_timeout_ms(total_timeout_ms: u64, obligation_count: usize) -> u64 {
    let count = u64::try_from(obligation_count.max(1)).unwrap_or(1);
    let fair_share = total_timeout_ms / count;
    fair_share.max(750)
}

fn l1_status_from_execution(execution: &L1ObligationExecution) -> EquivalenceEvidenceStatus {
    if execution.observed_difference > 0 {
        return EquivalenceEvidenceStatus::ObservedDifference;
    }
    if execution.no_difference_observed > 0 && execution.inconclusive == 0 && !execution.timed_out {
        return EquivalenceEvidenceStatus::NoDifferenceObserved;
    }

    EquivalenceEvidenceStatus::Inconclusive
}

pub(crate) fn l1_collect_proof_obligations(
    repo_root: &Path,
    scope: &L1RustBoundedScope,
    max_obligations: usize,
) -> Vec<L1ProofObligation> {
    let mut obligations = Vec::<L1ProofObligation>::new();

    for source_path in &scope.rust_files {
        let absolute_source_path = repo_root.join(source_path);
        let Some(manifest_path) = l1_nearest_manifest_path(repo_root, &absolute_source_path) else {
            continue;
        };
        let Ok(contents) = fs::read_to_string(&absolute_source_path) else {
            continue;
        };
        let manifest_path = l1_repo_relative_path(repo_root, &manifest_path);
        let source_path = source_path.replace('\\', "/");
        for (harness_name, line_number) in l1_extract_kani_proof_harnesses(&contents) {
            obligations.push(L1ProofObligation {
                manifest_path: manifest_path.clone(),
                source_path: source_path.clone(),
                harness_name,
                line_number,
            });
        }
    }

    obligations.sort_unstable_by(|left, right| {
        left.manifest_path
            .cmp(&right.manifest_path)
            .then_with(|| left.source_path.cmp(&right.source_path))
            .then_with(|| left.harness_name.cmp(&right.harness_name))
            .then_with(|| left.line_number.cmp(&right.line_number))
    });
    obligations.dedup_by(|left, right| {
        left.manifest_path == right.manifest_path && left.harness_name == right.harness_name
    });
    obligations.into_iter().take(max_obligations).collect()
}

fn l1_extract_kani_proof_harnesses(contents: &str) -> Vec<(String, usize)> {
    let lines = contents.lines().collect::<Vec<_>>();
    let mut harnesses = Vec::<(String, usize)>::new();

    for (index, line) in lines.iter().enumerate() {
        if !l1_is_kani_proof_attribute(line) {
            continue;
        }

        let mut candidate_index = index + 1;
        while candidate_index < lines.len() {
            let candidate_line = lines[candidate_index].trim();
            if candidate_line.is_empty() || candidate_line.starts_with("#[") {
                candidate_index += 1;
                continue;
            }
            if let Some(name) = l1_function_name_from_declaration(candidate_line) {
                harnesses.push((name, candidate_index + 1));
            }
            break;
        }
    }

    harnesses
}

fn l1_is_kani_proof_attribute(line: &str) -> bool {
    let compact = line
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    compact.starts_with("#[kani::proof")
}

fn l1_function_name_from_declaration(line: &str) -> Option<String> {
    let lowered = line.to_ascii_lowercase();
    let fn_index = lowered.find("fn ")?;
    let bytes = line.as_bytes();
    let mut start = fn_index + 3;
    while start < bytes.len() && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    let mut end = start;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    if end == start {
        return None;
    }

    Some(line[start..end].to_string())
}

fn l1_nearest_manifest_path(repo_root: &Path, source_path: &Path) -> Option<PathBuf> {
    let mut cursor = source_path.parent();
    while let Some(directory) = cursor {
        let candidate = directory.join("Cargo.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if directory == repo_root {
            break;
        }
        cursor = directory.parent();
    }

    None
}

fn l1_repo_relative_path(repo_root: &Path, absolute_path: &Path) -> String {
    match absolute_path.strip_prefix(repo_root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(_) => absolute_path.to_string_lossy().replace('\\', "/"),
    }
}

fn run_kani_obligation_with_timeout(
    repo_root: &Path,
    obligation: &L1ProofObligation,
    timeout: Duration,
) -> CommandRunOutcome {
    let mut command = ProcessCommand::new("cargo");
    command.arg("kani");
    command.arg("--manifest-path");
    command.arg(&obligation.manifest_path);
    command.arg("--harness");
    command.arg(&obligation.harness_name);
    command.current_dir(repo_root);

    run_command_with_timeout(command, timeout)
}

fn l1_obligation_outcome(
    run: CommandRunOutcome,
    obligation: &L1ProofObligation,
) -> (EquivalenceEvidenceStatus, Option<String>, bool) {
    let obligation_label = l1_obligation_label(obligation);
    match run {
        CommandRunOutcome::Completed(output) => {
            if output.status.success() {
                return (EquivalenceEvidenceStatus::NoDifferenceObserved, None, false);
            }

            let stdout = summarize_command_output(&output.stdout);
            let stderr = summarize_command_output(&output.stderr);
            let status = kani_failure_status(&stdout, &stderr);
            let line = first_non_empty_line(&stderr)
                .or_else(|| first_non_empty_line(&stdout))
                .unwrap_or_else(|| "kani_completed_with_non_zero_exit".to_string());
            (
                status,
                Some(format!(
                    "l1_obligation:{}:exit={}:{}",
                    obligation_label,
                    output.status.code().unwrap_or(-1),
                    line
                )),
                false,
            )
        }
        CommandRunOutcome::TimedOut(output) => {
            let timeout_line = output.and_then(|captured| {
                let stderr = summarize_command_output(&captured.stderr);
                let stdout = summarize_command_output(&captured.stdout);
                first_non_empty_line(&stderr).or_else(|| first_non_empty_line(&stdout))
            });
            (
                EquivalenceEvidenceStatus::Inconclusive,
                Some(format!(
                    "l1_obligation_timeout:{}:{}",
                    obligation_label,
                    timeout_line.unwrap_or_else(|| "timeout".to_string())
                )),
                true,
            )
        }
        CommandRunOutcome::SpawnFailed(error) | CommandRunOutcome::IoError(error) => (
            EquivalenceEvidenceStatus::Inconclusive,
            Some(format!(
                "l1_obligation_runtime_error:{obligation_label}:{error}"
            )),
            false,
        ),
    }
}

fn l1_obligation_label(obligation: &L1ProofObligation) -> String {
    format!(
        "{}::{}@{}:{}",
        obligation.manifest_path,
        obligation.harness_name,
        obligation.source_path,
        obligation.line_number
    )
}

fn run_kani_with_timeout(repo_root: &Path, policy: L1RustBoundedPolicy) -> CommandRunOutcome {
    let mut command = ProcessCommand::new("cargo");
    command.arg("kani");
    match policy.kani_mode {
        CliL1KaniMode::Probe => {
            command.arg("--version");
        }
        CliL1KaniMode::Check => {
            command.arg("--workspace");
            command.arg("--tests");
        }
    }
    command.current_dir(repo_root);

    run_command_with_timeout(command, Duration::from_millis(policy.timeout_ms))
}

pub(super) fn run_command_with_timeout(
    mut command: ProcessCommand,
    timeout: Duration,
) -> CommandRunOutcome {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());

    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return CommandRunOutcome::SpawnFailed(error.to_string()),
    };
    let child_pid = child.id();
    let (tx, rx) = sync_channel::<std::io::Result<Output>>(1);
    thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(timeout) {
        Ok(Ok(output)) => CommandRunOutcome::Completed(output),
        Ok(Err(error)) => CommandRunOutcome::IoError(error.to_string()),
        Err(RecvTimeoutError::Timeout) => {
            let _ = kill_process_tree(child_pid);
            match rx.recv_timeout(Duration::from_millis(500)) {
                Ok(Ok(output)) => CommandRunOutcome::TimedOut(Some(output)),
                Ok(Err(_)) | Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {
                    CommandRunOutcome::TimedOut(None)
                }
            }
        }
        Err(RecvTimeoutError::Disconnected) => {
            CommandRunOutcome::IoError("command worker disconnected before completion".to_string())
        }
    }
}

fn kill_process_tree(pid: u32) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let status = ProcessCommand::new("taskkill")
            .arg("/PID")
            .arg(pid.to_string())
            .arg("/T")
            .arg("/F")
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "taskkill returned non-zero exit for pid {pid}"
            )))
        }
    }

    #[cfg(not(windows))]
    {
        let term = ProcessCommand::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .status()?;
        if term.success() {
            return Ok(());
        }
        let kill = ProcessCommand::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status()?;
        if kill.success() {
            Ok(())
        } else {
            Err(std::io::Error::other(format!(
                "failed to terminate pid {pid}"
            )))
        }
    }
}

pub(super) fn summarize_command_output(output: &[u8]) -> String {
    const MAX_OUTPUT_CHARS: usize = 512;

    let normalized = String::from_utf8_lossy(output)
        .replace("\r\n", "\n")
        .trim()
        .to_string();
    if normalized.len() <= MAX_OUTPUT_CHARS {
        return normalized;
    }

    format!("{}...", &normalized[..MAX_OUTPUT_CHARS])
}

pub(super) fn first_non_empty_line(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn l1_kani_mode_label(mode: CliL1KaniMode) -> &'static str {
    match mode {
        CliL1KaniMode::Probe => "probe",
        CliL1KaniMode::Check => "check",
    }
}

fn kani_failure_status(stdout: &str, stderr: &str) -> EquivalenceEvidenceStatus {
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    if combined.contains("verification:- failed")
        || combined.contains("verification failed")
        || combined.contains("counterexample")
        || combined.contains("assertion failed")
    {
        EquivalenceEvidenceStatus::ObservedDifference
    } else {
        EquivalenceEvidenceStatus::Inconclusive
    }
}

pub(crate) fn l1_status_label(status: EquivalenceEvidenceStatus) -> &'static str {
    match status {
        EquivalenceEvidenceStatus::ObservedDifference => "l1_bounded_counterexample",
        EquivalenceEvidenceStatus::NoDifferenceObserved => "l1_bounded_no_counterexample",
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => "reference_snapshot_missing",
        EquivalenceEvidenceStatus::Inconclusive => "l1_bounded_inconclusive",
    }
}

pub(super) fn l1_summary(
    receipt: &EquivalenceEvidenceReceipt,
    delta: &SemanticDelta,
    reference_snapshot_id: &str,
    target_snapshot_id: &str,
) -> String {
    let structural = semantic_diff_summary(delta, reference_snapshot_id, target_snapshot_id);
    match receipt.status {
        EquivalenceEvidenceStatus::NoDifferenceObserved => format!(
            "L1 bounded formal backend (Kani) completed without reported counterexample under configured bounds. Structural baseline: {structural}"
        ),
        EquivalenceEvidenceStatus::ObservedDifference => format!(
            "L1 bounded formal backend (Kani) reported a bounded counterexample signal. Structural baseline: {structural}"
        ),
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => {
            "reference snapshot is missing; bounded formal backend was not executed".to_string()
        }
        EquivalenceEvidenceStatus::Inconclusive => format!(
            "L1 bounded formal backend (Kani) was inconclusive under configured bounds or environment constraints. Structural baseline: {structural}"
        ),
    }
}
