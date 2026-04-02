use super::{
    BTreeMap, BTreeSet, Duration, TheoremEvaluation, TheoremObligationStatus, TheoremPolicy,
    TheoremProofObligation,
};

mod layout;
mod parse;
mod smt;

use self::layout::{compute_gep_offset, layout_byte_size};
use self::parse::parse_function;
use self::smt::{compare_signatures, equivalence_script, run_smt_solver};

#[cfg(test)]
use self::smt::path_difference_formula;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(super) enum IvlEvaluationAttempt {
    Completed(TheoremEvaluation),
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Sort {
    Bool,
    BitVec(u16),
    Ptr,
    Aggregate(Vec<Sort>),
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LayoutType {
    Int(u16),
    Ptr,
    Array(u64, Box<LayoutType>),
    Struct(Vec<LayoutType>),
    Void,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Param {
    canonical_name: String,
    local_name: String,
    sort: Sort,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ValueRef {
    Local(String),
    Global(String),
    BoolConst(bool),
    BitVecConst { width: u16, value: u128 },
    Null,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Add,
    Sub,
    Mul,
    UDiv,
    SDiv,
    URem,
    SRem,
    And,
    Or,
    Xor,
    Shl,
    LShr,
    AShr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpPred {
    Eq,
    Ne,
    Ult,
    Ule,
    Ugt,
    Uge,
    Slt,
    Sle,
    Sgt,
    Sge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CastKind {
    ZExt,
    SExt,
    Trunc,
    BitCast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypedValue {
    sort: Sort,
    value: ValueRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CallSite {
    callee: String,
    return_sort: Sort,
    args: Vec<TypedValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SwitchCase {
    value: ValueRef,
    destination: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct InvokeTerminator {
    dest: Option<String>,
    call: CallSite,
    normal_label: String,
    unwind_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Rhs {
    Value {
        sort: Sort,
        value: ValueRef,
    },
    Binary {
        op: BinOp,
        sort: Sort,
        left: ValueRef,
        right: ValueRef,
    },
    Compare {
        pred: CmpPred,
        operand_sort: Sort,
        left: ValueRef,
        right: ValueRef,
    },
    Select {
        sort: Sort,
        cond: ValueRef,
        then_value: ValueRef,
        else_value: ValueRef,
    },
    Cast {
        kind: CastKind,
        from: Sort,
        to: Sort,
        value: ValueRef,
    },
    GetElementPtr {
        source_element: LayoutType,
        base: ValueRef,
        indices: Vec<i64>,
    },
    Load {
        sort: Sort,
        pointer: ValueRef,
    },
    Call(CallSite),
    ExtractValue {
        aggregate: ValueRef,
        indices: Vec<usize>,
    },
    LandingPad {
        sort: Sort,
    },
    Alloca {
        allocated: LayoutType,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Phi {
    dest: String,
    incomings: Vec<PhiIncoming>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PhiIncoming {
    predecessor: String,
    value: ValueRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Assign {
    dest: String,
    rhs: Rhs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Store {
    value_sort: Sort,
    value: ValueRef,
    pointer: ValueRef,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Instruction {
    Assign(Assign),
    Store(Store),
    Call(CallSite),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Terminator {
    Ret {
        value: Option<ValueRef>,
    },
    Br {
        destination: String,
    },
    CondBr {
        cond: ValueRef,
        then_label: String,
        else_label: String,
    },
    Switch {
        operand_sort: Sort,
        discriminator: ValueRef,
        default_label: String,
        cases: Vec<SwitchCase>,
    },
    Invoke(InvokeTerminator),
    Resume,
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Block {
    label: String,
    phis: Vec<Phi>,
    instructions: Vec<Instruction>,
    terminator: Terminator,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Function {
    params: Vec<Param>,
    return_sort: Sort,
    entry_label: String,
    blocks: BTreeMap<String, Block>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    BoolConst(bool),
    BitVecConst {
        width: u16,
        value: u128,
    },
    Var {
        name: String,
        sort: Sort,
    },
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Binary {
        op: BinOp,
        sort: Sort,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Compare {
        pred: CmpPred,
        operand_sort: Sort,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Select {
        cond: Box<Expr>,
        then_expr: Box<Expr>,
        else_expr: Box<Expr>,
    },
    Cast {
        kind: CastKind,
        from: Sort,
        to: Sort,
        expr: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PointerValue {
    base: String,
    offset_bytes: i64,
    local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SymbolicValue {
    Scalar { sort: Sort, expr: Expr },
    Pointer(PointerValue),
    Aggregate(Vec<SymbolicValue>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct MemoryCell {
    base: String,
    offset_bytes: i64,
    sort: Sort,
    local_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathWrite {
    cell: MemoryCell,
    value: SymbolicValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathSummary {
    guard: Expr,
    return_value: Option<SymbolicValue>,
    writes: Vec<PathWrite>,
    call_events: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
struct SummaryFeatures {
    pointers: bool,
    memory: bool,
    calls: bool,
    call_havoc: bool,
    local_allocas: bool,
}

impl SummaryFeatures {
    fn merge(&mut self, other: &SummaryFeatures) {
        self.pointers |= other.pointers;
        self.memory |= other.memory;
        self.calls |= other.calls;
        self.call_havoc |= other.call_havoc;
        self.local_allocas |= other.local_allocas;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FunctionSummary {
    params: Vec<Param>,
    return_sort: Sort,
    paths: Vec<PathSummary>,
    features: SummaryFeatures,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct MemoryState {
    cells: BTreeMap<MemoryCell, SymbolicValue>,
    havoc_epochs: BTreeMap<String, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExecutionState {
    block_label: String,
    predecessor: Option<String>,
    environment: BTreeMap<String, SymbolicValue>,
    memory: MemoryState,
    guard: Expr,
    visited_blocks: BTreeMap<String, usize>,
    written_cells: BTreeSet<MemoryCell>,
    call_events: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SmtOutcome {
    Sat,
    Unsat,
    Unknown,
}

const IVL_MAX_PATHS: usize = 24;
const IVL_MAX_BLOCK_REVISITS: usize = 1;
const IVL_POINTER_WIDTH_BITS: u16 = 64;

pub(super) fn ivl_evaluate_obligation(
    obligation: &TheoremProofObligation,
    reference_ir: &str,
    target_ir: &str,
    policy: &TheoremPolicy,
    timeout: Duration,
) -> IvlEvaluationAttempt {
    let reference = match parse_function(reference_ir) {
        Ok(function) => function,
        Err(error) => {
            return IvlEvaluationAttempt::Unavailable {
                reason: format!("reference_ivl_parse:{error}"),
            };
        }
    };
    let target = match parse_function(target_ir) {
        Ok(function) => function,
        Err(error) => {
            return IvlEvaluationAttempt::Unavailable {
                reason: format!("target_ivl_parse:{error}"),
            };
        }
    };
    let reference = match summarize_function(&reference) {
        Ok(summary) => summary,
        Err(error) => {
            return IvlEvaluationAttempt::Unavailable {
                reason: format!("reference_ivl_summary:{error}"),
            };
        }
    };
    let target = match summarize_function(&target) {
        Ok(summary) => summary,
        Err(error) => {
            return IvlEvaluationAttempt::Unavailable {
                reason: format!("target_ivl_summary:{error}"),
            };
        }
    };
    let features = merged_features(&reference, &target);
    if let Err(witness) = compare_signatures(&reference, &target) {
        let mut assumptions = ivl_feature_assumptions(&features);
        assumptions.push(
            "signature mismatch is treated as an observable semantic-state difference under the current contract"
                .to_string(),
        );
        return IvlEvaluationAttempt::Completed(TheoremEvaluation {
            status: TheoremObligationStatus::Refuted,
            solver: Some("theorem_llvm_ssa_bv_v1".to_string()),
            assumptions,
            witness: Some(witness),
        });
    }

    let script = match equivalence_script(obligation, &reference, &target) {
        Ok(script) => script,
        Err(error) => {
            return IvlEvaluationAttempt::Unavailable {
                reason: format!("ivl_script_build:{error}"),
            };
        }
    };
    match run_smt_solver(policy, obligation, &script, timeout) {
        SmtOutcome::Unsat => {
            let mut assumptions = ivl_feature_assumptions(&features);
            assumptions.push(
                "equivalence was checked over explicit symbolic return-state, observable memory-write, and call-summary formulas in QF_BV"
                    .to_string(),
            );
            IvlEvaluationAttempt::Completed(TheoremEvaluation {
                status: TheoremObligationStatus::Proved,
                solver: Some("theorem_llvm_ssa_bv_v1".to_string()),
                assumptions,
                witness: None,
            })
        }
        SmtOutcome::Sat => {
            let mut assumptions = ivl_feature_assumptions(&features);
            assumptions.push(
                "counterexample search was performed over explicit symbolic return-state, observable memory-write, and call-summary formulas in QF_BV"
                    .to_string(),
            );
            IvlEvaluationAttempt::Completed(TheoremEvaluation {
                status: TheoremObligationStatus::Refuted,
                solver: Some("theorem_llvm_ssa_bv_v1".to_string()),
                assumptions,
                witness: Some(format!("ivl_ssa_counterexample:{}", obligation.id)),
            })
        }
        SmtOutcome::Unknown => IvlEvaluationAttempt::Unavailable {
            reason: format!("ivl_smt_unknown:{}", obligation.id),
        },
    }
}

fn merged_features(reference: &FunctionSummary, target: &FunctionSummary) -> SummaryFeatures {
    let mut features = reference.features.clone();
    features.merge(&target.features);
    features
}

fn ivl_feature_assumptions(features: &SummaryFeatures) -> Vec<String> {
    let mut assumptions =
        vec!["compiler-emitted LLVM IR was lowered to a typed SSA/IVL summary".to_string()];
    if features.pointers {
        assumptions.push(
            "pointer values are summarized by opaque-pointer base identity plus constant byte offsets"
                .to_string(),
        );
    }
    if features.memory {
        assumptions.push(
            "memory summaries track typed load/store cells keyed by canonical pointer summary and explicit LLVM access type"
                .to_string(),
        );
    }
    if features.calls {
        assumptions.push(
            "call summaries are bounded and structural: they record callee identity, argument summaries, and pointer-havoc footprint instead of interprocedural proof"
                .to_string(),
        );
        assumptions.push(
            "identical summarized calls are treated as observationally stable across compared revisions under the current contract"
                .to_string(),
        );
    }
    if features.local_allocas {
        assumptions.push(
            "alloca-backed local memory is tracked for intra-procedural reasoning but excluded from observable side-effect summaries unless surfaced through returned values or observable writes"
                .to_string(),
        );
    }
    assumptions.push(
        "the proof remains bounded to supported LLVM instructions, acyclic control flow, constant-offset pointer arithmetic, and deterministic path exploration"
            .to_string(),
    );
    assumptions
}

fn summarize_function(function: &Function) -> Result<FunctionSummary, String> {
    let mut states = vec![ExecutionState {
        block_label: function.entry_label.clone(),
        predecessor: None,
        environment: initial_environment(function)?,
        memory: MemoryState::default(),
        guard: Expr::BoolConst(true),
        visited_blocks: BTreeMap::new(),
        written_cells: BTreeSet::new(),
        call_events: Vec::new(),
    }];
    let mut paths = Vec::<PathSummary>::new();
    let mut features = initial_features(function);
    while let Some(state) = states.pop() {
        if paths.len() >= IVL_MAX_PATHS {
            return Err("path_budget_exceeded".to_string());
        }
        advance_state(function, state, &mut states, &mut paths, &mut features)?;
    }
    if paths.is_empty() {
        return Err("no_returning_paths".to_string());
    }

    Ok(FunctionSummary {
        params: function.params.clone(),
        return_sort: function.return_sort.clone(),
        paths,
        features,
    })
}

fn initial_features(function: &Function) -> SummaryFeatures {
    let mut features = SummaryFeatures::default();
    features.pointers |= function
        .params
        .iter()
        .any(|param| sort_contains_pointer(&param.sort));
    features.pointers |= sort_contains_pointer(&function.return_sort);
    features
}

fn sort_contains_pointer(sort: &Sort) -> bool {
    match sort {
        Sort::Ptr => true,
        Sort::Aggregate(items) => items.iter().any(sort_contains_pointer),
        Sort::Bool | Sort::BitVec(_) | Sort::Void => false,
    }
}

fn initial_environment(function: &Function) -> Result<BTreeMap<String, SymbolicValue>, String> {
    let mut environment = BTreeMap::<String, SymbolicValue>::new();
    for param in &function.params {
        environment.insert(param.local_name.clone(), param_symbolic_value(param)?);
    }
    Ok(environment)
}

fn param_symbolic_value(param: &Param) -> Result<SymbolicValue, String> {
    symbolic_param_value(&param.sort, &param.canonical_name)
}

fn symbolic_param_value(sort: &Sort, canonical_name: &str) -> Result<SymbolicValue, String> {
    match sort {
        Sort::Bool | Sort::BitVec(_) => Ok(SymbolicValue::Scalar {
            sort: sort.clone(),
            expr: Expr::Var {
                name: canonical_name.to_string(),
                sort: sort.clone(),
            },
        }),
        Sort::Ptr => Ok(SymbolicValue::Pointer(PointerValue {
            base: format!("param:{canonical_name}"),
            offset_bytes: 0,
            local_only: false,
        })),
        Sort::Aggregate(items) => Ok(SymbolicValue::Aggregate(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    symbolic_param_value(item, &format!("{canonical_name}_{index}"))
                })
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Sort::Void => Err("void_parameter_unsupported".to_string()),
    }
}

#[allow(clippy::too_many_lines)]
fn advance_state(
    function: &Function,
    state: ExecutionState,
    states: &mut Vec<ExecutionState>,
    paths: &mut Vec<PathSummary>,
    features: &mut SummaryFeatures,
) -> Result<(), String> {
    let block = function
        .blocks
        .get(&state.block_label)
        .ok_or_else(|| format!("missing_block:{}", state.block_label))?;
    let mut visited_blocks = state.visited_blocks.clone();
    let visits = visited_blocks.entry(block.label.clone()).or_insert(0);
    *visits += 1;
    if *visits > IVL_MAX_BLOCK_REVISITS {
        return Err(format!("backedge_or_loop_detected:{}", block.label));
    }

    let mut environment = state.environment.clone();
    let mut memory = state.memory.clone();
    let mut written_cells = state.written_cells.clone();
    let mut call_events = state.call_events.clone();
    for phi in &block.phis {
        let predecessor = state
            .predecessor
            .as_deref()
            .ok_or_else(|| format!("phi_without_predecessor:{}", block.label))?;
        let Some(incoming) = phi
            .incomings
            .iter()
            .find(|incoming| incoming.predecessor == predecessor)
        else {
            return Err(format!(
                "phi_missing_incoming:{}:{}",
                block.label, predecessor
            ));
        };
        environment.insert(
            phi.dest.clone(),
            resolve_value_ref_typed(&incoming.value, None, &environment)?,
        );
    }
    for instruction in &block.instructions {
        match instruction {
            Instruction::Assign(assign) => {
                let value = evaluate_rhs(
                    &assign.rhs,
                    &assign.dest,
                    &environment,
                    &mut memory,
                    &mut written_cells,
                    &mut call_events,
                    features,
                )?;
                environment.insert(assign.dest.clone(), value);
            }
            Instruction::Store(store) => execute_store(
                store,
                &environment,
                &mut memory,
                &mut written_cells,
                features,
            )?,
            Instruction::Call(call) => {
                let _ = execute_call(
                    call,
                    &environment,
                    &mut memory,
                    &mut written_cells,
                    &mut call_events,
                    features,
                )?;
            }
        }
    }

    match &block.terminator {
        Terminator::Ret { value } => {
            let return_value = match value {
                Some(value) => Some(resolve_value_ref_typed(value, None, &environment)?),
                None => None,
            };
            let writes = written_cells
                .iter()
                .filter(|cell| !cell.local_only)
                .map(|cell| {
                    let value = memory.cells.get(cell).cloned().ok_or_else(|| {
                        format!("missing_memory_cell:{}:{}", cell.base, cell.offset_bytes)
                    })?;
                    Ok(PathWrite {
                        cell: cell.clone(),
                        value,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            paths.push(PathSummary {
                guard: state.guard,
                return_value,
                writes,
                call_events,
            });
        }
        Terminator::Br { destination } => {
            states.push(ExecutionState {
                block_label: destination.clone(),
                predecessor: Some(block.label.clone()),
                environment,
                memory,
                guard: state.guard,
                visited_blocks,
                written_cells,
                call_events,
            });
        }
        Terminator::CondBr {
            cond,
            then_label,
            else_label,
        } => {
            let cond = expect_bool(resolve_value_ref_typed(
                cond,
                Some(&Sort::Bool),
                &environment,
            )?)?;
            states.push(ExecutionState {
                block_label: then_label.clone(),
                predecessor: Some(block.label.clone()),
                environment: environment.clone(),
                memory: memory.clone(),
                guard: Expr::And(Box::new(state.guard.clone()), Box::new(cond.clone())),
                visited_blocks: visited_blocks.clone(),
                written_cells: written_cells.clone(),
                call_events: call_events.clone(),
            });
            states.push(ExecutionState {
                block_label: else_label.clone(),
                predecessor: Some(block.label.clone()),
                environment,
                memory,
                guard: Expr::And(Box::new(state.guard), Box::new(Expr::Not(Box::new(cond)))),
                visited_blocks,
                written_cells,
                call_events,
            });
        }
        Terminator::Switch {
            operand_sort,
            discriminator,
            default_label,
            cases,
        } => {
            let discriminator =
                resolve_value_ref_typed(discriminator, Some(operand_sort), &environment)?;
            let mut case_guards = Vec::<Expr>::with_capacity(cases.len());
            for case in cases {
                let case_value =
                    resolve_value_ref_typed(&case.value, Some(operand_sort), &environment)?;
                let case_guard =
                    equality_guard_for_values(discriminator.clone(), case_value, operand_sort)?;
                states.push(ExecutionState {
                    block_label: case.destination.clone(),
                    predecessor: Some(block.label.clone()),
                    environment: environment.clone(),
                    memory: memory.clone(),
                    guard: Expr::And(Box::new(state.guard.clone()), Box::new(case_guard.clone())),
                    visited_blocks: visited_blocks.clone(),
                    written_cells: written_cells.clone(),
                    call_events: call_events.clone(),
                });
                case_guards.push(case_guard);
            }

            let mut default_guard_terms = vec![state.guard];
            default_guard_terms.extend(
                case_guards
                    .into_iter()
                    .map(|guard| Expr::Not(Box::new(guard))),
            );
            states.push(ExecutionState {
                block_label: default_label.clone(),
                predecessor: Some(block.label.clone()),
                environment,
                memory,
                guard: and_exprs(default_guard_terms),
                visited_blocks,
                written_cells,
                call_events,
            });
        }
        Terminator::Invoke(invoke) => {
            let result = execute_call(
                &invoke.call,
                &environment,
                &mut memory,
                &mut written_cells,
                &mut call_events,
                features,
            )?;
            let outcome_seed = call_events
                .last()
                .cloned()
                .unwrap_or_else(|| format!("invoke:{}", invoke.call.callee));
            let normal_guard = fresh_bool_expr(
                "invoke",
                &format!(
                    "{};normal={};unwind={}",
                    outcome_seed, invoke.normal_label, invoke.unwind_label
                ),
            );
            let mut normal_environment = environment.clone();
            if let Some(dest) = invoke.dest.as_deref() {
                let Some(value) = result.clone() else {
                    return Err(format!("void_invoke_cannot_bind:{dest}"));
                };
                normal_environment.insert(dest.to_string(), value);
            }
            states.push(ExecutionState {
                block_label: invoke.normal_label.clone(),
                predecessor: Some(block.label.clone()),
                environment: normal_environment,
                memory: memory.clone(),
                guard: Expr::And(
                    Box::new(state.guard.clone()),
                    Box::new(normal_guard.clone()),
                ),
                visited_blocks: visited_blocks.clone(),
                written_cells: written_cells.clone(),
                call_events: call_events.clone(),
            });
            states.push(ExecutionState {
                block_label: invoke.unwind_label.clone(),
                predecessor: Some(block.label.clone()),
                environment,
                memory,
                guard: Expr::And(
                    Box::new(state.guard),
                    Box::new(Expr::Not(Box::new(normal_guard))),
                ),
                visited_blocks,
                written_cells,
                call_events,
            });
        }
        Terminator::Resume | Terminator::Unreachable => {}
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn evaluate_rhs(
    rhs: &Rhs,
    dest: &str,
    environment: &BTreeMap<String, SymbolicValue>,
    memory: &mut MemoryState,
    written_cells: &mut BTreeSet<MemoryCell>,
    call_events: &mut Vec<String>,
    features: &mut SummaryFeatures,
) -> Result<SymbolicValue, String> {
    match rhs {
        Rhs::Value { sort, value } => resolve_value_ref_typed(value, Some(sort), environment),
        Rhs::Binary {
            op,
            sort,
            left,
            right,
        } => {
            let left = expect_scalar_expr(
                resolve_value_ref_typed(left, Some(sort), environment)?,
                sort,
            )?;
            let right = expect_scalar_expr(
                resolve_value_ref_typed(right, Some(sort), environment)?,
                sort,
            )?;
            Ok(SymbolicValue::Scalar {
                sort: sort.clone(),
                expr: Expr::Binary {
                    op: *op,
                    sort: sort.clone(),
                    left: Box::new(left),
                    right: Box::new(right),
                },
            })
        }
        Rhs::Compare {
            pred,
            operand_sort,
            left,
            right,
        } => {
            let expr = if *operand_sort == Sort::Ptr {
                pointer_compare_expr(
                    *pred,
                    &expect_pointer(resolve_value_ref_typed(
                        left,
                        Some(operand_sort),
                        environment,
                    )?)?,
                    &expect_pointer(resolve_value_ref_typed(
                        right,
                        Some(operand_sort),
                        environment,
                    )?)?,
                )
            } else {
                let left = expect_scalar_expr(
                    resolve_value_ref_typed(left, Some(operand_sort), environment)?,
                    operand_sort,
                )?;
                let right = expect_scalar_expr(
                    resolve_value_ref_typed(right, Some(operand_sort), environment)?,
                    operand_sort,
                )?;
                Expr::Compare {
                    pred: *pred,
                    operand_sort: operand_sort.clone(),
                    left: Box::new(left),
                    right: Box::new(right),
                }
            };
            Ok(SymbolicValue::Scalar {
                sort: Sort::Bool,
                expr,
            })
        }
        Rhs::Select {
            cond,
            then_value,
            else_value,
            ..
        } => {
            let cond = expect_bool(resolve_value_ref_typed(
                cond,
                Some(&Sort::Bool),
                environment,
            )?)?;
            let then_value = resolve_value_ref_typed(then_value, None, environment)?;
            let else_value = resolve_value_ref_typed(else_value, None, environment)?;
            select_value(cond, then_value, else_value)
        }
        Rhs::Cast {
            kind,
            from,
            to,
            value,
        } => {
            let value = resolve_value_ref_typed(value, Some(from), environment)?;
            cast_value(*kind, from, to, value)
        }
        Rhs::GetElementPtr {
            source_element,
            base,
            indices,
        } => {
            features.pointers = true;
            let base = expect_pointer(resolve_value_ref_typed(
                base,
                Some(&Sort::Ptr),
                environment,
            )?)?;
            let offset = compute_gep_offset(source_element, indices)?;
            Ok(SymbolicValue::Pointer(PointerValue {
                base: base.base,
                offset_bytes: base.offset_bytes + offset,
                local_only: base.local_only,
            }))
        }
        Rhs::Load { sort, pointer } => {
            features.pointers = true;
            features.memory = true;
            let pointer = expect_pointer(resolve_value_ref_typed(
                pointer,
                Some(&Sort::Ptr),
                environment,
            )?)?;
            load_from_memory(sort, &pointer, memory)
        }
        Rhs::Call(call) => execute_call(
            call,
            environment,
            memory,
            written_cells,
            call_events,
            features,
        )?
        .ok_or_else(|| format!("void_call_cannot_bind:{}", call.callee)),
        Rhs::ExtractValue { aggregate, indices } => {
            let aggregate = resolve_value_ref_typed(aggregate, None, environment)?;
            extract_value(aggregate, indices)
        }
        Rhs::LandingPad { sort } => {
            features.pointers |= sort_contains_pointer(sort);
            fresh_symbolic_value("landingpad", dest, sort)
        }
        Rhs::Alloca { allocated } => {
            features.pointers = true;
            features.memory = true;
            let _ = layout_byte_size(allocated)?;
            features.local_allocas = true;
            Ok(SymbolicValue::Pointer(PointerValue {
                base: format!("alloca:{dest}"),
                offset_bytes: 0,
                local_only: true,
            }))
        }
    }
}

fn execute_store(
    store: &Store,
    environment: &BTreeMap<String, SymbolicValue>,
    memory: &mut MemoryState,
    written_cells: &mut BTreeSet<MemoryCell>,
    features: &mut SummaryFeatures,
) -> Result<(), String> {
    features.pointers = true;
    features.memory = true;
    let value = resolve_value_ref_typed(&store.value, Some(&store.value_sort), environment)?;
    let pointer = expect_pointer(resolve_value_ref_typed(
        &store.pointer,
        Some(&Sort::Ptr),
        environment,
    )?)?;
    let cell = MemoryCell {
        base: pointer.base,
        offset_bytes: pointer.offset_bytes,
        sort: store.value_sort.clone(),
        local_only: pointer.local_only,
    };
    memory.cells.insert(cell.clone(), value);
    let _ = written_cells.insert(cell);
    Ok(())
}

fn execute_call(
    call: &CallSite,
    environment: &BTreeMap<String, SymbolicValue>,
    memory: &mut MemoryState,
    written_cells: &mut BTreeSet<MemoryCell>,
    call_events: &mut Vec<String>,
    features: &mut SummaryFeatures,
) -> Result<Option<SymbolicValue>, String> {
    features.calls = true;
    let resolved_args = call
        .args
        .iter()
        .map(|arg| resolve_value_ref_typed(&arg.value, Some(&arg.sort), environment))
        .collect::<Result<Vec<_>, _>>()?;
    let pointer_bases = resolved_args
        .iter()
        .filter_map(pointer_base)
        .collect::<BTreeSet<_>>();
    let memory_snapshot = pointer_bases
        .iter()
        .flat_map(|base| memory_snapshot_entries(memory, base))
        .collect::<Vec<_>>();
    let mut key_parts = vec![
        format!("callee={}", call.callee),
        format!("ret={}", sort_key(&call.return_sort)),
        format!(
            "args={}",
            resolved_args
                .iter()
                .map(value_key)
                .collect::<Vec<_>>()
                .join("|")
        ),
    ];
    if !memory_snapshot.is_empty() {
        key_parts.push(format!("mem={}", memory_snapshot.join("|")));
    }
    if !pointer_bases.is_empty() {
        key_parts.push(format!(
            "ptr_bases={}",
            pointer_bases.iter().cloned().collect::<Vec<_>>().join("|")
        ));
    }
    let call_key = key_parts.join(";");
    call_events.push(call_key.clone());

    if !pointer_bases.is_empty() {
        features.call_havoc = true;
        for base in &pointer_bases {
            memory.havoc_base(base);
            written_cells.retain(|cell| cell.base != *base);
        }
    }

    if matches!(call.return_sort, Sort::Void) {
        return Ok(None);
    }

    Ok(Some(fresh_symbolic_value(
        "call",
        &call_key,
        &call.return_sort,
    )?))
}

fn extract_value(value: SymbolicValue, indices: &[usize]) -> Result<SymbolicValue, String> {
    let mut current = value;
    for index in indices {
        let SymbolicValue::Aggregate(items) = current else {
            return Err(format!("extractvalue_non_aggregate_index:{index}"));
        };
        current = items
            .get(*index)
            .cloned()
            .ok_or_else(|| format!("extractvalue_out_of_bounds:{index}"))?;
    }
    Ok(current)
}

fn load_from_memory(
    sort: &Sort,
    pointer: &PointerValue,
    memory: &mut MemoryState,
) -> Result<SymbolicValue, String> {
    let cell = MemoryCell {
        base: pointer.base.clone(),
        offset_bytes: pointer.offset_bytes,
        sort: sort.clone(),
        local_only: pointer.local_only,
    };
    if let Some(value) = memory.cells.get(&cell) {
        return Ok(value.clone());
    }
    let epoch = memory.havoc_epoch(&cell.base);
    fresh_symbolic_value(
        "load",
        &format!(
            "{}:{}:{}:epoch{}",
            cell.base,
            cell.offset_bytes,
            sort_key(&cell.sort),
            epoch
        ),
        sort,
    )
}

fn select_value(
    cond: Expr,
    then_value: SymbolicValue,
    else_value: SymbolicValue,
) -> Result<SymbolicValue, String> {
    if then_value == else_value {
        return Ok(then_value);
    }
    match (then_value, else_value) {
        (
            SymbolicValue::Scalar {
                sort: left_sort,
                expr: left,
            },
            SymbolicValue::Scalar {
                sort: right_sort,
                expr: right,
            },
        ) if left_sort == right_sort => Ok(SymbolicValue::Scalar {
            sort: left_sort,
            expr: Expr::Select {
                cond: Box::new(cond),
                then_expr: Box::new(left),
                else_expr: Box::new(right),
            },
        }),
        (SymbolicValue::Aggregate(left), SymbolicValue::Aggregate(right))
            if left.len() == right.len() =>
        {
            Ok(SymbolicValue::Aggregate(
                left.into_iter()
                    .zip(right)
                    .map(|(left, right)| select_value(cond.clone(), left, right))
                    .collect::<Result<Vec<_>, _>>()?,
            ))
        }
        (SymbolicValue::Pointer(_), SymbolicValue::Pointer(_)) => {
            Err("pointer_select_unsupported".to_string())
        }
        _ => Err("select_value_type_mismatch".to_string()),
    }
}

fn cast_value(
    kind: CastKind,
    from: &Sort,
    to: &Sort,
    value: SymbolicValue,
) -> Result<SymbolicValue, String> {
    if *from == Sort::Ptr && *to == Sort::Ptr && matches!(kind, CastKind::BitCast) {
        return Ok(value);
    }
    let expr = expect_scalar_expr(value, from)?;
    Ok(SymbolicValue::Scalar {
        sort: to.clone(),
        expr: Expr::Cast {
            kind,
            from: from.clone(),
            to: to.clone(),
            expr: Box::new(expr),
        },
    })
}

fn resolve_value_ref_typed(
    value: &ValueRef,
    expected: Option<&Sort>,
    environment: &BTreeMap<String, SymbolicValue>,
) -> Result<SymbolicValue, String> {
    let resolved = match value {
        ValueRef::Local(name) => environment
            .get(name)
            .cloned()
            .ok_or_else(|| format!("unknown_local:{name}"))?,
        ValueRef::Global(name) => {
            if expected == Some(&Sort::Ptr) || expected.is_none() {
                SymbolicValue::Pointer(PointerValue {
                    base: format!("global:{name}"),
                    offset_bytes: 0,
                    local_only: false,
                })
            } else {
                return Err(format!("unsupported_global_value:{name}"));
            }
        }
        ValueRef::BoolConst(value) => SymbolicValue::Scalar {
            sort: Sort::Bool,
            expr: Expr::BoolConst(*value),
        },
        ValueRef::BitVecConst { width, value } => SymbolicValue::Scalar {
            sort: Sort::BitVec(*width),
            expr: Expr::BitVecConst {
                width: *width,
                value: *value,
            },
        },
        ValueRef::Null => SymbolicValue::Pointer(PointerValue {
            base: "null".to_string(),
            offset_bytes: 0,
            local_only: false,
        }),
    };
    if let Some(expected) = expected
        && resolved.sort() != *expected
    {
        return Err(format!(
            "type_mismatch:{}!={}",
            sort_key(&resolved.sort()),
            sort_key(expected)
        ));
    }
    Ok(resolved)
}

fn expect_scalar_expr(value: SymbolicValue, expected: &Sort) -> Result<Expr, String> {
    let SymbolicValue::Scalar { sort, expr } = value else {
        return Err(format!("expected_scalar:{}", sort_key(expected)));
    };
    if sort != *expected {
        return Err(format!(
            "scalar_sort_mismatch:{}!={}",
            sort_key(&sort),
            sort_key(expected)
        ));
    }
    Ok(expr)
}

fn expect_bool(value: SymbolicValue) -> Result<Expr, String> {
    expect_scalar_expr(value, &Sort::Bool)
}

fn expect_pointer(value: SymbolicValue) -> Result<PointerValue, String> {
    let SymbolicValue::Pointer(pointer) = value else {
        return Err("expected_pointer".to_string());
    };
    Ok(pointer)
}

impl SymbolicValue {
    fn sort(&self) -> Sort {
        match self {
            SymbolicValue::Scalar { sort, .. } => sort.clone(),
            SymbolicValue::Pointer(_) => Sort::Ptr,
            SymbolicValue::Aggregate(items) => {
                Sort::Aggregate(items.iter().map(SymbolicValue::sort).collect())
            }
        }
    }
}

impl MemoryState {
    fn havoc_epoch(&self, base: &str) -> u32 {
        self.havoc_epochs.get(base).copied().unwrap_or(0)
    }

    fn havoc_base(&mut self, base: &str) {
        let epoch = self.havoc_epochs.entry(base.to_string()).or_insert(0);
        *epoch = epoch.saturating_add(1);
        self.cells.retain(|cell, _| cell.base != base);
    }
}

fn pointer_base(value: &SymbolicValue) -> Option<String> {
    let SymbolicValue::Pointer(pointer) = value else {
        return None;
    };
    Some(pointer.base.clone())
}

fn memory_snapshot_entries(memory: &MemoryState, base: &str) -> Vec<String> {
    memory
        .cells
        .iter()
        .filter(|(cell, _)| cell.base == base)
        .map(|(cell, value)| {
            format!(
                "{}:{}:{}={}",
                cell.base,
                cell.offset_bytes,
                sort_key(&cell.sort),
                value_key(value)
            )
        })
        .collect()
}

fn fresh_symbolic_value(kind: &str, seed: &str, sort: &Sort) -> Result<SymbolicValue, String> {
    match sort {
        Sort::Bool | Sort::BitVec(_) => Ok(SymbolicValue::Scalar {
            sort: sort.clone(),
            expr: Expr::Var {
                name: fresh_name(kind, seed, sort),
                sort: sort.clone(),
            },
        }),
        Sort::Ptr => Ok(SymbolicValue::Pointer(PointerValue {
            base: format!("freshptr:{}", fresh_name(kind, seed, &Sort::BitVec(64))),
            offset_bytes: 0,
            local_only: false,
        })),
        Sort::Aggregate(items) => Ok(SymbolicValue::Aggregate(
            items
                .iter()
                .enumerate()
                .map(|(index, item)| fresh_symbolic_value(kind, &format!("{seed}:{index}"), item))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Sort::Void => Err("void_fresh_value".to_string()),
    }
}

fn fresh_bool_expr(kind: &str, seed: &str) -> Expr {
    Expr::Var {
        name: fresh_name(kind, seed, &Sort::Bool),
        sort: Sort::Bool,
    }
}

fn fresh_name(kind: &str, seed: &str, sort: &Sort) -> String {
    format!(
        "ivl_{kind}_{:016x}",
        stable_hash(format!("{}|{}|{}", kind, sort_key(sort), seed).as_bytes())
    )
}

fn stable_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0100_0000_01b3;
    let mut hash = OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn pointer_compare_expr(pred: CmpPred, left: &PointerValue, right: &PointerValue) -> Expr {
    match pred {
        CmpPred::Eq => pointer_equal_expr(left, right),
        CmpPred::Ne => Expr::Not(Box::new(pointer_equal_expr(left, right))),
        _ => Expr::BoolConst(false),
    }
}

fn pointer_equal_expr(left: &PointerValue, right: &PointerValue) -> Expr {
    if left.base == right.base {
        if left.offset_bytes == right.offset_bytes {
            return Expr::BoolConst(true);
        }
        return Expr::BoolConst(false);
    }
    and_exprs(vec![
        Expr::Compare {
            pred: CmpPred::Eq,
            operand_sort: Sort::BitVec(IVL_POINTER_WIDTH_BITS),
            left: Box::new(pointer_base_expr(&left.base)),
            right: Box::new(pointer_base_expr(&right.base)),
        },
        Expr::Compare {
            pred: CmpPred::Eq,
            operand_sort: Sort::BitVec(IVL_POINTER_WIDTH_BITS),
            left: Box::new(pointer_offset_expr(left.offset_bytes)),
            right: Box::new(pointer_offset_expr(right.offset_bytes)),
        },
    ])
}

fn pointer_base_expr(base: &str) -> Expr {
    Expr::Var {
        name: pointer_base_var_name(base),
        sort: Sort::BitVec(IVL_POINTER_WIDTH_BITS),
    }
}

fn pointer_offset_expr(offset_bytes: i64) -> Expr {
    Expr::BitVecConst {
        width: IVL_POINTER_WIDTH_BITS,
        value: i128::from(offset_bytes).cast_unsigned(),
    }
}

fn pointer_base_var_name(base: &str) -> String {
    format!("ptr_base_{:016x}", stable_hash(base.as_bytes()))
}

fn equality_guard_for_values(
    left: SymbolicValue,
    right: SymbolicValue,
    sort: &Sort,
) -> Result<Expr, String> {
    match sort {
        Sort::Ptr => Ok(pointer_equal_expr(
            &expect_pointer(left)?,
            &expect_pointer(right)?,
        )),
        Sort::Bool | Sort::BitVec(_) => Ok(Expr::Compare {
            pred: CmpPred::Eq,
            operand_sort: sort.clone(),
            left: Box::new(expect_scalar_expr(left, sort)?),
            right: Box::new(expect_scalar_expr(right, sort)?),
        }),
        Sort::Aggregate(_) | Sort::Void => Err(format!(
            "unsupported_switch_operand_sort:{}",
            sort_key(sort)
        )),
    }
}

fn and_exprs(exprs: Vec<Expr>) -> Expr {
    let mut iter = exprs.into_iter();
    let Some(first) = iter.next() else {
        return Expr::BoolConst(true);
    };
    iter.fold(first, |left, right| {
        Expr::And(Box::new(left), Box::new(right))
    })
}

fn value_key(value: &SymbolicValue) -> String {
    match value {
        SymbolicValue::Scalar { sort, expr } => {
            format!("{}:{}", sort_key(sort), expr_key(expr))
        }
        SymbolicValue::Pointer(pointer) => {
            format!("ptr:{}:{}", pointer.base, pointer.offset_bytes)
        }
        SymbolicValue::Aggregate(items) => format!(
            "agg({})",
            items.iter().map(value_key).collect::<Vec<_>>().join("|")
        ),
    }
}

fn expr_key(expr: &Expr) -> String {
    match expr {
        Expr::BoolConst(value) => format!("bool:{value}"),
        Expr::BitVecConst { width, value } => format!("bv{width}:{value}"),
        Expr::Var { name, sort } => format!("var:{}:{}", sort_key(sort), name),
        Expr::Not(inner) => format!("not({})", expr_key(inner)),
        Expr::And(left, right) => format!("and({},{})", expr_key(left), expr_key(right)),
        Expr::Binary {
            op,
            sort,
            left,
            right,
        } => format!(
            "bin({op:?},{}:{},{})",
            sort_key(sort),
            expr_key(left),
            expr_key(right)
        ),
        Expr::Compare {
            pred,
            operand_sort,
            left,
            right,
        } => format!(
            "cmp({pred:?},{}:{},{})",
            sort_key(operand_sort),
            expr_key(left),
            expr_key(right)
        ),
        Expr::Select {
            cond,
            then_expr,
            else_expr,
        } => format!(
            "ite({},{},{})",
            expr_key(cond),
            expr_key(then_expr),
            expr_key(else_expr)
        ),
        Expr::Cast {
            kind,
            from,
            to,
            expr,
        } => format!(
            "cast({kind:?},{}->{}, {})",
            sort_key(from),
            sort_key(to),
            expr_key(expr)
        ),
    }
}

fn sort_key(sort: &Sort) -> String {
    match sort {
        Sort::Bool => "bool".to_string(),
        Sort::BitVec(width) => format!("bv{width}"),
        Sort::Ptr => "ptr".to_string(),
        Sort::Aggregate(items) => format!(
            "agg({})",
            items.iter().map(sort_key).collect::<Vec<_>>().join(",")
        ),
        Sort::Void => "void".to_string(),
    }
}
