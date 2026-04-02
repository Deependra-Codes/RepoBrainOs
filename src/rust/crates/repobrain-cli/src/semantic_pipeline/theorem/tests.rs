use super::*;
use crate::semantic_pipeline::L2BehaviorProfile;

fn signal(
    id: &str,
    name: &str,
    path: &str,
    line: u32,
    literal_signature: &str,
) -> L2FunctionSignal {
    signal_with_details(
        id,
        name,
        path,
        line,
        literal_signature,
        Some("Cargo.toml"),
        "rust",
    )
}

fn signal_with_details(
    id: &str,
    name: &str,
    path: &str,
    line: u32,
    literal_signature: &str,
    manifest_path: Option<&str>,
    language: &str,
) -> L2FunctionSignal {
    let behavior_profile = L2BehaviorProfile {
        declaration_signature: "fn(i32)->i32".to_string(),
        call_signature: "map|collect".to_string(),
        control_signature: "if|match".to_string(),
        literal_signature: literal_signature.to_string(),
        normalized_window_hash: "window-abcd".to_string(),
    };
    let behavior_signature = format!(
        "lang=rust|decl={}|calls={}|control={}|literals={}|window_hash={}|imports=std::fmt|symbol_context=function:1|evidence=syntax_confirmed",
        behavior_profile.declaration_signature,
        behavior_profile.call_signature,
        behavior_profile.control_signature,
        behavior_profile.literal_signature,
        behavior_profile.normalized_window_hash
    );

    L2FunctionSignal {
        id: id.to_string(),
        name: name.to_string(),
        name_key: name.replace('_', " "),
        relative_path: path.to_string(),
        manifest_path: manifest_path.map(ToString::to_string),
        language: language.to_string(),
        line_number: line,
        import_signature: "std::fmt".to_string(),
        reverse_import_signature: "crate::entry".to_string(),
        symbol_context_signature: "function:1".to_string(),
        behavior_profile,
        behavior_signature,
    }
}

fn obligation() -> TheoremProofObligation {
    obligation_with(
        "thm_demo",
        "pair:reference@src/lib.rs:10=>target@src/lib.rs:11",
        vec![
            "stable filesystem".to_string(),
            "alignment_strategy=signature_similarity".to_string(),
        ],
    )
}

fn obligation_with(
    id: &str,
    source_pair: &str,
    assumptions: Vec<String>,
) -> TheoremProofObligation {
    TheoremProofObligation {
        id: id.to_string(),
        source_pair: source_pair.to_string(),
        contract_id: "repo-equivalence-v1".to_string(),
        assumptions,
        encoding_hash: "0123456789abcdef".to_string(),
    }
}

fn translation_ordering_context() -> TheoremExecutionContext {
    TheoremExecutionContext {
        reference_map: [
            (
                "reference_exact@src/lib.rs:10".to_string(),
                signal(
                    "reference_exact@src/lib.rs:10",
                    "compute_total",
                    "src/lib.rs",
                    10,
                    "1|2|3",
                ),
            ),
            (
                "reference_lower@src/lib.rs:20".to_string(),
                signal(
                    "reference_lower@src/lib.rs:20",
                    "compute_total",
                    "src/lib.rs",
                    20,
                    "1|2|3",
                ),
            ),
            (
                "reference_script@src/tool.py:3".to_string(),
                signal_with_details(
                    "reference_script@src/tool.py:3",
                    "helper",
                    "src/tool.py",
                    3,
                    "print",
                    None,
                    "python",
                ),
            ),
        ]
        .into_iter()
        .collect(),
        target_map: [
            (
                "target_exact@src/lib.rs:10".to_string(),
                signal(
                    "target_exact@src/lib.rs:10",
                    "compute_total",
                    "src/lib.rs",
                    10,
                    "1|2|3",
                ),
            ),
            (
                "target_lower@src/lib.rs:20".to_string(),
                signal(
                    "target_lower@src/lib.rs:20",
                    "compute_total",
                    "src/lib.rs",
                    20,
                    "1|2|3",
                ),
            ),
            (
                "target_script@src/tool.py:3".to_string(),
                signal_with_details(
                    "target_script@src/tool.py:3",
                    "helper",
                    "src/tool.py",
                    3,
                    "print",
                    None,
                    "python",
                ),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

#[test]
fn theorem_smt_script_encodes_symbolic_field_constraints() {
    let reference = signal(
        "reference@src/lib.rs:10",
        "compute_total",
        "src/lib.rs",
        10,
        "1|2|3",
    );
    let target = signal(
        "target@src/lib.rs:11",
        "compute_total",
        "src/lib.rs",
        11,
        "1|2|3",
    );
    let model = theorem_smt_pair_model(&obligation(), &reference, &target);
    let script = theorem_smt_script(&model);

    assert!(script.contains("(set-logic QF_LIA)"));
    assert!(script.contains("(declare-const contract_tag Int)"));
    assert!(script.contains("(declare-const ref_declaration_tag Int)"));
    assert!(script.contains("(define-fun declaration_eq () Bool"));
    assert!(script.contains("(define-fun semantic_score () Int"));
    assert!(script.contains("(define-fun equivalence_ok () Bool"));
    assert!(script.contains("(assert (not equivalence_ok))"));
    assert!(script.contains("(check-sat)"));
}

#[test]
fn theorem_pair_smt_score_drops_when_literal_signature_changes() {
    let reference = signal(
        "reference@src/lib.rs:10",
        "compute_total",
        "src/lib.rs",
        10,
        "1|2|3",
    );
    let target_same = signal(
        "target@src/lib.rs:10",
        "compute_total",
        "src/lib.rs",
        10,
        "1|2|3",
    );
    let target_changed = signal(
        "target@src/lib.rs:10",
        "compute_total",
        "src/lib.rs",
        10,
        "9|9|9",
    );

    let base_model = theorem_smt_pair_model(&obligation(), &reference, &target_same);
    let changed_model = theorem_smt_pair_model(&obligation(), &reference, &target_changed);

    assert!(theorem_pair_smt_score(&base_model) > theorem_pair_smt_score(&changed_model));
}

#[test]
fn theorem_ground_model_outcome_short_circuits_when_model_is_ground() {
    let reference = signal(
        "reference@src/lib.rs:10",
        "compute_total",
        "src/lib.rs",
        10,
        "1|2|3",
    );
    let target_same = signal(
        "target@src/lib.rs:11",
        "compute_total",
        "src/lib.rs",
        11,
        "1|2|3",
    );
    let target_changed = signal(
        "target@src/lib.rs:11",
        "compute_total",
        "src/lib.rs",
        11,
        "9|9|9",
    );

    let proved_model = theorem_smt_pair_model(&obligation(), &reference, &target_same);
    let refuted_model = theorem_smt_pair_model(&obligation(), &reference, &target_changed);

    assert_eq!(
        theorem_ground_model_outcome(&proved_model),
        Some(SmtCheckResult::Unsat)
    );
    assert_eq!(
        theorem_ground_model_outcome(&refuted_model),
        Some(SmtCheckResult::Sat)
    );
}

#[test]
fn theorem_parse_llvm_module_functions_extracts_debug_metadata() {
    let module = r#"
define i32 @foo() !dbg !10 {
entry:
  ret i32 0, !dbg !11
}

!10 = distinct !DISubprogram(name: "foo", scope: !12, file: !12, line: 7, type: !13)
!11 = !DILocation(line: 8, scope: !10)
!12 = !DIFile(filename: "src/lib.rs", directory: "/tmp/demo")
"#;

    let functions = theorem_parse_llvm_module_functions(module);
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].symbol_name, "foo");
    assert_eq!(functions[0].subprogram_name.as_deref(), Some("foo"));
    assert_eq!(
        functions[0].source_path.as_deref(),
        Some("/tmp/demo/src/lib.rs")
    );
    assert_eq!(functions[0].line_number, Some(7));
    assert!(functions[0].ir.contains("define i32 @foo()"));
}

#[test]
fn theorem_pair_obligations_for_translation_prefers_ready_exact_path_pairs() {
    let exact_ready = obligation_with(
        "thm_exact_ready",
        "pair:reference_exact@src/lib.rs:10=>target_exact@src/lib.rs:10",
        vec![
            "alignment_strategy=exact_path_name".to_string(),
            "alignment_score=100".to_string(),
        ],
    );
    let lower_rank = obligation_with(
        "thm_lower_rank",
        "pair:reference_lower@src/lib.rs:20=>target_lower@src/lib.rs:20",
        vec![
            "alignment_strategy=signature_similarity".to_string(),
            "alignment_score=65".to_string(),
        ],
    );
    let non_rust = obligation_with(
        "thm_non_rust",
        "pair:reference_script@src/tool.py:3=>target_script@src/tool.py:3",
        vec![
            "alignment_strategy=exact_path_name".to_string(),
            "alignment_score=100".to_string(),
        ],
    );
    let context = translation_ordering_context();

    let obligations = [lower_rank.clone(), non_rust.clone(), exact_ready.clone()];
    let ordered = theorem_pair_obligations_for_translation(&obligations, Some(&context), 3);
    let ordered_ids = ordered
        .iter()
        .map(|obligation| obligation.id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        ordered_ids,
        vec!["thm_exact_ready", "thm_lower_rank", "thm_non_rust"]
    );
}

#[test]
fn theorem_probe_external_tool_reports_missing_binary() {
    let result = theorem_probe_external_tool(
        "definitely-missing-theorem-tool-binary",
        &std::env::temp_dir(),
        Duration::from_millis(50),
    );

    assert!(result.is_err());
    assert!(
        result
            .err()
            .is_some_and(|reason| reason.starts_with("spawn_failed:"))
    );
}

#[test]
fn theorem_per_obligation_timeout_accounts_for_parallel_slots() {
    assert_eq!(theorem_per_obligation_timeout_ms(30_000, 128, 1), 400);
    assert_eq!(theorem_per_obligation_timeout_ms(30_000, 128, 12), 2_727);
    assert_eq!(theorem_per_obligation_timeout_ms(5_000, 4, 8), 5_000);
}
