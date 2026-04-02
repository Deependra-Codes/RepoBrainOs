use super::super::l1::{run_command_with_timeout, summarize_command_output};
use super::super::{CliTheoremSolverMode, CommandRunOutcome, fs};
use super::{
    BTreeMap, BinOp, CastKind, CmpPred, Duration, Expr, FunctionSummary, IVL_POINTER_WIDTH_BITS,
    Param, PathSummary, PathWrite, PointerValue, SmtOutcome, Sort, SymbolicValue, TheoremPolicy,
    TheoremProofObligation, pointer_base_var_name, pointer_equal_expr, sort_key,
};

pub(super) fn compare_signatures(
    reference: &FunctionSummary,
    target: &FunctionSummary,
) -> Result<(), String> {
    if reference.params.len() != target.params.len() {
        return Err(format!(
            "ivl_param_arity_mismatch:{}:{}",
            reference.params.len(),
            target.params.len()
        ));
    }
    for (index, (left, right)) in reference
        .params
        .iter()
        .zip(target.params.iter())
        .enumerate()
    {
        if left.sort != right.sort {
            return Err(format!(
                "ivl_param_sort_mismatch:arg{index}:{}!={}",
                sort_key(&left.sort),
                sort_key(&right.sort)
            ));
        }
    }
    if reference.return_sort != target.return_sort {
        return Err(format!(
            "ivl_return_sort_mismatch:{}!={}",
            sort_key(&reference.return_sort),
            sort_key(&target.return_sort)
        ));
    }
    Ok(())
}

pub(super) fn equivalence_script(
    obligation: &TheoremProofObligation,
    reference: &FunctionSummary,
    target: &FunctionSummary,
) -> Result<String, String> {
    let mut lines = vec![
        "(set-logic QF_BV)".to_string(),
        "(set-option :produce-models true)".to_string(),
        format!("; obligation={}", obligation.id),
        format!("; encoding_hash={}", obligation.encoding_hash),
    ];
    let mut vars = BTreeMap::<String, Sort>::new();
    collect_summary_vars(reference, &mut vars);
    collect_summary_vars(target, &mut vars);
    for param in &reference.params {
        collect_param_vars(param, &mut vars);
    }
    for (name, sort) in vars {
        lines.push(format!("(declare-const {name} {})", smt_sort(&sort)?));
    }
    for (prefix, summary) in [("ref", reference), ("tgt", target)] {
        for (index, path) in summary.paths.iter().enumerate() {
            lines.push(format!(
                "(define-fun {prefix}_guard_{index} () Bool {})",
                expr_to_smt(&path.guard)?
            ));
        }
        let guard_names = (0..summary.paths.len())
            .map(|index| format!("{prefix}_guard_{index}"))
            .collect::<Vec<_>>();
        lines.push(format!(
            "(define-fun {prefix}_defined () Bool {})",
            or_terms(&guard_names)
        ));
    }

    let mut counterexample_terms = vec![
        "(and ref_defined (not tgt_defined))".to_string(),
        "(and tgt_defined (not ref_defined))".to_string(),
    ];
    for (ref_index, ref_path) in reference.paths.iter().enumerate() {
        for (tgt_index, tgt_path) in target.paths.iter().enumerate() {
            let diff = path_difference_formula(ref_path, tgt_path)?;
            if diff == "false" {
                continue;
            }
            counterexample_terms.push(format!(
                "(and ref_guard_{ref_index} tgt_guard_{tgt_index} {diff})"
            ));
        }
    }
    lines.push(format!("(assert {})", or_terms(&counterexample_terms)));
    lines.push("(check-sat)".to_string());
    lines.push("(get-model)".to_string());
    Ok(lines.join("\n"))
}

fn collect_param_vars(param: &Param, vars: &mut BTreeMap<String, Sort>) {
    match &param.sort {
        Sort::Bool | Sort::BitVec(_) => {
            vars.entry(param.canonical_name.clone())
                .or_insert_with(|| param.sort.clone());
        }
        Sort::Ptr => {
            vars.entry(pointer_base_var_name(&format!(
                "param:{}",
                param.canonical_name
            )))
            .or_insert(Sort::BitVec(IVL_POINTER_WIDTH_BITS));
        }
        Sort::Aggregate(items) => {
            for (index, item) in items.iter().enumerate() {
                collect_param_vars(
                    &Param {
                        canonical_name: format!("{}_{}", param.canonical_name, index),
                        local_name: String::new(),
                        sort: item.clone(),
                    },
                    vars,
                );
            }
        }
        Sort::Void => {}
    }
}

fn collect_summary_vars(summary: &FunctionSummary, vars: &mut BTreeMap<String, Sort>) {
    for path in &summary.paths {
        collect_expr_vars(&path.guard, vars);
        if let Some(return_value) = &path.return_value {
            collect_value_vars(return_value, vars);
        }
        for write in &path.writes {
            collect_value_vars(&write.value, vars);
        }
    }
}

fn collect_expr_vars(expr: &Expr, vars: &mut BTreeMap<String, Sort>) {
    match expr {
        Expr::Var { name, sort } => {
            vars.entry(name.clone()).or_insert_with(|| sort.clone());
        }
        Expr::Not(inner) => collect_expr_vars(inner, vars),
        Expr::And(left, right)
        | Expr::Binary { left, right, .. }
        | Expr::Compare { left, right, .. } => {
            collect_expr_vars(left, vars);
            collect_expr_vars(right, vars);
        }
        Expr::Select {
            cond,
            then_expr,
            else_expr,
        } => {
            collect_expr_vars(cond, vars);
            collect_expr_vars(then_expr, vars);
            collect_expr_vars(else_expr, vars);
        }
        Expr::Cast { expr, .. } => collect_expr_vars(expr, vars),
        Expr::BoolConst(_) | Expr::BitVecConst { .. } => {}
    }
}

fn collect_value_vars(value: &SymbolicValue, vars: &mut BTreeMap<String, Sort>) {
    match value {
        SymbolicValue::Scalar { expr, .. } => collect_expr_vars(expr, vars),
        SymbolicValue::Pointer(pointer) => {
            vars.entry(pointer_base_var_name(&pointer.base))
                .or_insert(Sort::BitVec(IVL_POINTER_WIDTH_BITS));
        }
        SymbolicValue::Aggregate(items) => {
            for item in items {
                collect_value_vars(item, vars);
            }
        }
    }
}

pub(super) fn path_difference_formula(
    reference: &PathSummary,
    target: &PathSummary,
) -> Result<String, String> {
    let mut terms = Vec::new();
    let return_diff = optional_value_difference(
        reference.return_value.as_ref(),
        target.return_value.as_ref(),
    )?;
    if return_diff != "false" {
        terms.push(return_diff);
    }
    let write_diff = writes_difference_formula(&reference.writes, &target.writes)?;
    if write_diff != "false" {
        terms.push(write_diff);
    }
    if reference.call_events != target.call_events {
        terms.push("true".to_string());
    }
    Ok(or_terms(&terms))
}

fn writes_difference_formula(
    reference: &[PathWrite],
    target: &[PathWrite],
) -> Result<String, String> {
    if reference.len() != target.len() {
        return Ok("true".to_string());
    }
    let mut terms = Vec::new();
    for (left, right) in reference.iter().zip(target.iter()) {
        if left.cell != right.cell {
            return Ok("true".to_string());
        }
        let diff = value_difference_formula(&left.value, &right.value)?;
        if diff != "false" {
            terms.push(diff);
        }
    }
    Ok(or_terms(&terms))
}

fn optional_value_difference(
    reference: Option<&SymbolicValue>,
    target: Option<&SymbolicValue>,
) -> Result<String, String> {
    match (reference, target) {
        (None, None) => Ok("false".to_string()),
        (Some(_), None) | (None, Some(_)) => Ok("true".to_string()),
        (Some(left), Some(right)) => value_difference_formula(left, right),
    }
}

fn value_difference_formula(
    reference: &SymbolicValue,
    target: &SymbolicValue,
) -> Result<String, String> {
    if reference == target {
        return Ok("false".to_string());
    }
    match (reference, target) {
        (
            SymbolicValue::Scalar {
                sort: left_sort,
                expr: left,
            },
            SymbolicValue::Scalar {
                sort: right_sort,
                expr: right,
            },
        ) if left_sort == right_sort => Ok(format!(
            "(not (= {} {}))",
            expr_to_smt(left)?,
            expr_to_smt(right)?
        )),
        (SymbolicValue::Pointer(left), SymbolicValue::Pointer(right)) => {
            Ok(format!("(not {})", pointer_equal_formula(left, right)?))
        }
        (SymbolicValue::Aggregate(left), SymbolicValue::Aggregate(right))
            if left.len() == right.len() =>
        {
            let mut terms = Vec::new();
            for (left, right) in left.iter().zip(right.iter()) {
                let diff = value_difference_formula(left, right)?;
                if diff != "false" {
                    terms.push(diff);
                }
            }
            Ok(or_terms(&terms))
        }
        _ => Ok("true".to_string()),
    }
}

fn pointer_equal_formula(left: &PointerValue, right: &PointerValue) -> Result<String, String> {
    expr_to_smt(&pointer_equal_expr(left, right))
}

fn expr_to_smt(expr: &Expr) -> Result<String, String> {
    match expr {
        Expr::BoolConst(true) => Ok("true".to_string()),
        Expr::BoolConst(false) => Ok("false".to_string()),
        Expr::BitVecConst { width, value } => Ok(bitvec_literal(*width, *value)),
        Expr::Var { name, .. } => Ok(name.clone()),
        Expr::Not(inner) => Ok(format!("(not {})", expr_to_smt(inner)?)),
        Expr::And(left, right) => Ok(format!(
            "(and {} {})",
            expr_to_smt(left)?,
            expr_to_smt(right)?
        )),
        Expr::Binary {
            op,
            sort,
            left,
            right,
        } => {
            let left = expr_to_smt(left)?;
            let right = expr_to_smt(right)?;
            Ok(match (sort, op) {
                (Sort::Bool, BinOp::And) => format!("(and {left} {right})"),
                (Sort::Bool, BinOp::Or) => format!("(or {left} {right})"),
                (Sort::Bool, BinOp::Xor) => format!("(xor {left} {right})"),
                (Sort::BitVec(_), BinOp::Add) => format!("(bvadd {left} {right})"),
                (Sort::BitVec(_), BinOp::Sub) => format!("(bvsub {left} {right})"),
                (Sort::BitVec(_), BinOp::Mul) => format!("(bvmul {left} {right})"),
                (Sort::BitVec(_), BinOp::UDiv) => format!("(bvudiv {left} {right})"),
                (Sort::BitVec(_), BinOp::SDiv) => format!("(bvsdiv {left} {right})"),
                (Sort::BitVec(_), BinOp::URem) => format!("(bvurem {left} {right})"),
                (Sort::BitVec(_), BinOp::SRem) => format!("(bvsrem {left} {right})"),
                (Sort::BitVec(_), BinOp::And) => format!("(bvand {left} {right})"),
                (Sort::BitVec(_), BinOp::Or) => format!("(bvor {left} {right})"),
                (Sort::BitVec(_), BinOp::Xor) => format!("(bvxor {left} {right})"),
                (Sort::BitVec(_), BinOp::Shl) => format!("(bvshl {left} {right})"),
                (Sort::BitVec(_), BinOp::LShr) => format!("(bvlshr {left} {right})"),
                (Sort::BitVec(_), BinOp::AShr) => format!("(bvashr {left} {right})"),
                _ => return Err("unsupported_binary_smt_encoding".to_string()),
            })
        }
        Expr::Compare {
            pred,
            operand_sort,
            left,
            right,
        } => {
            let left = expr_to_smt(left)?;
            let right = expr_to_smt(right)?;
            Ok(match (operand_sort, pred) {
                (Sort::Bool | Sort::BitVec(_), CmpPred::Eq) => format!("(= {left} {right})"),
                (Sort::Bool | Sort::BitVec(_), CmpPred::Ne) => {
                    format!("(not (= {left} {right}))")
                }
                (Sort::BitVec(_), CmpPred::Ult) => format!("(bvult {left} {right})"),
                (Sort::BitVec(_), CmpPred::Ule) => format!("(bvule {left} {right})"),
                (Sort::BitVec(_), CmpPred::Ugt) => format!("(bvugt {left} {right})"),
                (Sort::BitVec(_), CmpPred::Uge) => format!("(bvuge {left} {right})"),
                (Sort::BitVec(_), CmpPred::Slt) => format!("(bvslt {left} {right})"),
                (Sort::BitVec(_), CmpPred::Sle) => format!("(bvsle {left} {right})"),
                (Sort::BitVec(_), CmpPred::Sgt) => format!("(bvsgt {left} {right})"),
                (Sort::BitVec(_), CmpPred::Sge) => format!("(bvsge {left} {right})"),
                _ => return Err("unsupported_compare_smt_encoding".to_string()),
            })
        }
        Expr::Select {
            cond,
            then_expr,
            else_expr,
        } => Ok(format!(
            "(ite {} {} {})",
            expr_to_smt(cond)?,
            expr_to_smt(then_expr)?,
            expr_to_smt(else_expr)?
        )),
        Expr::Cast {
            kind,
            from,
            to,
            expr,
        } => cast_to_smt(*kind, from, to, expr),
    }
}

fn cast_to_smt(kind: CastKind, from: &Sort, to: &Sort, expr: &Expr) -> Result<String, String> {
    let source = expr_to_smt(expr)?;
    match (kind, from, to) {
        (CastKind::ZExt, Sort::Bool, Sort::BitVec(width)) => Ok(format!(
            "(ite {source} {} {})",
            bitvec_literal(*width, 1),
            bitvec_literal(*width, 0)
        )),
        (CastKind::SExt, Sort::Bool, Sort::BitVec(width)) => Ok(format!(
            "(ite {source} {} {})",
            bitvec_literal(*width, all_ones(*width)),
            bitvec_literal(*width, 0)
        )),
        (CastKind::ZExt, Sort::BitVec(from_width), Sort::BitVec(to_width))
            if to_width > from_width =>
        {
            Ok(format!(
                "((_ zero_extend {}) {source})",
                to_width - from_width
            ))
        }
        (CastKind::SExt, Sort::BitVec(from_width), Sort::BitVec(to_width))
            if to_width > from_width =>
        {
            Ok(format!(
                "((_ sign_extend {}) {source})",
                to_width - from_width
            ))
        }
        (CastKind::Trunc, Sort::BitVec(from_width), Sort::BitVec(to_width))
            if to_width < from_width =>
        {
            Ok(format!("((_ extract {} 0) {source})", to_width - 1))
        }
        (CastKind::Trunc, Sort::BitVec(_), Sort::Bool) => {
            Ok(format!("(= ((_ extract 0 0) {source}) #b1)"))
        }
        (CastKind::BitCast, Sort::BitVec(from_width), Sort::BitVec(to_width))
            if from_width == to_width =>
        {
            Ok(source)
        }
        _ => Err("unsupported_cast_smt_encoding".to_string()),
    }
}

fn bitvec_literal(width: u16, value: u128) -> String {
    let bits = usize::from(width);
    let masked = if bits >= 128 {
        value
    } else {
        value & ((1_u128 << bits) - 1)
    };
    format!("#b{masked:0bits$b}")
}

fn all_ones(width: u16) -> u128 {
    if width >= 128 {
        u128::MAX
    } else {
        (1_u128 << u32::from(width)) - 1
    }
}

fn smt_sort(sort: &Sort) -> Result<String, String> {
    match sort {
        Sort::Bool => Ok("Bool".to_string()),
        Sort::BitVec(width) => Ok(format!("(_ BitVec {width})")),
        Sort::Ptr | Sort::Aggregate(_) | Sort::Void => {
            Err("unsupported_non_scalar_smt_sort".to_string())
        }
    }
}

pub(super) fn run_smt_solver(
    policy: &TheoremPolicy,
    obligation: &TheoremProofObligation,
    script: &str,
    timeout: Duration,
) -> SmtOutcome {
    if !matches!(policy.solver_mode, CliTheoremSolverMode::SmtZ3) {
        return SmtOutcome::Unknown;
    }
    let solver_binary = policy.solver_path.as_ref().map_or_else(
        || "z3".to_string(),
        |path| path.to_string_lossy().to_string(),
    );
    let script_path = std::env::temp_dir().join(format!("repobrain-ivl-{}.smt2", obligation.id));
    if fs::write(&script_path, script).is_err() {
        return SmtOutcome::Unknown;
    }
    let mut command = std::process::Command::new(&solver_binary);
    command.arg("-smt2");
    command.arg(format!("-T:{}", timeout.as_secs().max(1)));
    command.arg(&script_path);
    let run = run_command_with_timeout(command, timeout);
    let _ = fs::remove_file(script_path);
    match run {
        CommandRunOutcome::Completed(output) => {
            let combined = format!(
                "{}\n{}",
                summarize_command_output(&output.stdout),
                summarize_command_output(&output.stderr)
            )
            .to_ascii_lowercase();
            if combined.contains("unsat") {
                SmtOutcome::Unsat
            } else if combined.contains("sat") {
                SmtOutcome::Sat
            } else {
                SmtOutcome::Unknown
            }
        }
        CommandRunOutcome::TimedOut(_)
        | CommandRunOutcome::SpawnFailed(_)
        | CommandRunOutcome::IoError(_) => SmtOutcome::Unknown,
    }
}

fn or_terms(terms: &[String]) -> String {
    if terms.is_empty() {
        return "false".to_string();
    }
    if terms.len() == 1 {
        return terms[0].clone();
    }
    format!("(or {})", terms.join(" "))
}
