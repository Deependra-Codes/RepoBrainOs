use super::*;

fn policy() -> TheoremPolicy {
    TheoremPolicy {
        max_obligations: 32,
        max_candidate_functions: 32,
        timeout_ms: 2_000,
        flaky_retries: 0,
        stability_runs: 1,
        max_parallelism: 1,
        solver_mode: super::super::CliTheoremSolverMode::SmtZ3,
        solver_path: None,
        solver_timeout_ms: 2_000,
        solver_fallback_relational: true,
        replay_out: None,
        persist_replay: false,
        translation_validation: true,
        alive2_path: None,
        translation_max_obligations: 8,
        translation_timeout_ms: 2_000,
        translation_ir_mode: super::super::CliTranslationIrMode::RequireCompilerIr,
        translation_run_status_mode: super::super::CliTranslationRunStatusMode::SidecarOnly,
    }
}

fn obligation() -> TheoremProofObligation {
    TheoremProofObligation {
        id: "thm_demo".to_string(),
        source_pair: "pair:reference=>target".to_string(),
        contract_id: "repo-equivalence-v1".to_string(),
        assumptions: vec!["stable filesystem".to_string()],
        encoding_hash: "0123456789abcdef".to_string(),
    }
}

fn must<T>(result: Result<T, String>, context: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{context}:{error}"),
    }
}

#[test]
fn parses_and_summarizes_branching_function() {
    let ir = r"
define i32 @foo(i32 %a, i32 %b) {
entry:
  %cond = icmp eq i32 %a, %b
  br i1 %cond, label %same, label %diff
same:
  ret i32 %a
diff:
  %sum = add i32 %a, %b
  ret i32 %sum
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.params.len(), 2);
    assert_eq!(summary.paths.len(), 2);
    assert_eq!(summary.return_sort, Sort::BitVec(32));
    assert!(!summary.features.memory);
    assert!(!summary.features.calls);
}

#[test]
fn emits_bitvector_equivalence_script() {
    let ir = r"
define i32 @foo(i32 %a) {
entry:
  %x = add i32 %a, 1
  ret i32 %x
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");
    let script = must(
        equivalence_script(&obligation(), &summary, &summary),
        "script",
    );

    assert!(script.contains("(set-logic QF_BV)"));
    assert!(script.contains("(declare-const arg0 (_ BitVec 32))"));
    assert!(script.contains("(define-fun ref_guard_0 () Bool"));
    assert!(script.contains("(assert (or"));
}

#[test]
fn supports_extractvalue_and_unreachable_blocks() {
    let ir = r"
define internal i32 @helper(i32 %v) {
start:
  %0 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %v, i32 1)
  %sum = extractvalue { i32, i1 } %0, 0
  %overflow = extractvalue { i32, i1 } %0, 1
  br i1 %overflow, label %panic, label %ok
ok:
  ret i32 %sum
panic:
  call void @panic(ptr @msg)
  unreachable
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.paths.len(), 1);
    assert!(summary.features.calls);
    assert_eq!(summary.return_sort, Sort::BitVec(32));
}

#[test]
fn summarizes_load_store_gep_and_call_effects() {
    let ir = r"
define i32 @sample(ptr %p, ptr %q, i1 %flag) {
start:
  %a = load i32, ptr %q, align 4
  %field = getelementptr inbounds i8, ptr %q, i64 4
  %b = call i32 @read(ptr %field, ptr @meta)
  br i1 %flag, label %then, label %else
then:
  %sum = add i32 %a, %b
  %next = call i32 @helper(i32 %sum)
  store i32 %next, ptr %p, align 4
  br label %done
else:
  %diff = sub i32 %a, %b
  %slot = getelementptr inbounds i8, ptr %p, i64 4
  store i32 %diff, ptr %slot, align 4
  br label %done
done:
  %ret = load i32, ptr %p, align 4
  ret i32 %ret
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.paths.len(), 2);
    assert!(summary.features.pointers);
    assert!(summary.features.memory);
    assert!(summary.features.calls);
    assert!(summary.features.call_havoc);
    let call_counts = summary
        .paths
        .iter()
        .map(|path| path.call_events.len())
        .collect::<BTreeSet<_>>();
    assert_eq!(call_counts, BTreeSet::from([1_usize, 2_usize]));
    assert!(
        summary
            .paths
            .iter()
            .flat_map(|path| path.writes.iter())
            .any(|write| write.cell.offset_bytes == 0 || write.cell.offset_bytes == 4)
    );
}

#[test]
fn path_difference_formula_detects_memory_write_change() {
    let left = r"
define i32 @foo(ptr %p) {
entry:
  store i32 1, ptr %p
  ret i32 0
}
";
    let right = r"
define i32 @foo(ptr %p) {
entry:
  %slot = getelementptr inbounds i8, ptr %p, i64 4
  store i32 1, ptr %slot
  ret i32 0
}
";
    let left_function = must(parse_function(left), "left should parse");
    let left_summary = must(summarize_function(&left_function), "left");
    let right_function = must(parse_function(right), "right should parse");
    let right_summary = must(summarize_function(&right_function), "right");
    let diff = must(
        path_difference_formula(&left_summary.paths[0], &right_summary.paths[0]),
        "diff",
    );

    assert_eq!(diff, "true");
}

#[test]
fn reports_mismatched_signatures_as_refuted() {
    let left = r"
define i32 @foo(i32 %a) {
entry:
  ret i32 %a
}
";
    let right = r"
define i64 @foo(i64 %a) {
entry:
  ret i64 %a
}
";
    let outcome = ivl_evaluate_obligation(
        &obligation(),
        left,
        right,
        &policy(),
        Duration::from_secs(1),
    );

    match outcome {
        IvlEvaluationAttempt::Completed(result) => {
            assert_eq!(result.status, TheoremObligationStatus::Refuted);
        }
        IvlEvaluationAttempt::Unavailable { reason } => {
            panic!("unexpected unavailable: {reason}")
        }
    }
}

#[test]
fn supports_switch_branching_paths() {
    let ir = r"
define i32 @chooser(i32 %state, i32 %a, i32 %b) {
entry:
  switch i32 %state, label %default [
    i32 0, label %zero
    i32 1, label %one
  ]
zero:
  ret i32 %a
one:
  ret i32 %b
default:
  %sum = add i32 %a, %b
  ret i32 %sum
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.paths.len(), 3);
    assert_eq!(summary.return_sort, Sort::BitVec(32));
    assert!(!summary.features.memory);
    assert!(!summary.features.calls);
}

#[test]
fn supports_invoke_landingpad_and_resume_paths() {
    let ir = r"
define i32 @sample(ptr %p, i1 %flag) personality ptr @__CxxFrameHandler3 {
entry:
  %result = invoke i32 @work(ptr %p)
      to label %ok unwind label %lpad
ok:
  ret i32 %result
lpad:
  %lp = landingpad { ptr, i32 }
      cleanup
  %tag = extractvalue { ptr, i32 } %lp, 1
  br i1 %flag, label %recover, label %resume
recover:
  ret i32 %tag
resume:
  resume { ptr, i32 } %lp
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.paths.len(), 2);
    assert_eq!(summary.return_sort, Sort::BitVec(32));
    assert!(summary.features.calls);
    assert!(summary.features.pointers);
}

#[test]
fn supports_struct_gep_offsets_and_writes() {
    let ir = r"
define i32 @write_field(ptr %p, i32 %value) {
entry:
  %field = getelementptr inbounds { i8, i32 }, ptr %p, i64 0, i32 1
  store i32 %value, ptr %field, align 4
  %loaded = load i32, ptr %field, align 4
  ret i32 %loaded
}
";
    let function = must(parse_function(ir), "function should parse");
    let summary = must(summarize_function(&function), "summary should build");

    assert_eq!(summary.paths.len(), 1);
    assert!(summary.features.memory);
    assert_eq!(summary.paths[0].writes.len(), 1);
    assert_eq!(summary.paths[0].writes[0].cell.offset_bytes, 4);
}
