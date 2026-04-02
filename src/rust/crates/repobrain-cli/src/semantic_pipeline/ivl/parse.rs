use super::{
    Assign, BTreeMap, BinOp, Block, CallSite, CastKind, CmpPred, Function, Instruction,
    InvokeTerminator, LayoutType, Param, Phi, PhiIncoming, Rhs, Sort, Store, SwitchCase,
    Terminator, TypedValue, ValueRef,
};

pub(super) fn parse_function(ir: &str) -> Result<Function, String> {
    let lines = ir.lines().map(str::trim).collect::<Vec<_>>();
    let Some(header) = lines.iter().find(|line| line.starts_with("define ")) else {
        return Err("missing_define".to_string());
    };
    let at_index = header
        .find('@')
        .ok_or_else(|| "missing_function_marker".to_string())?;
    let header_tokens = split_ir_tokens_top_level(&header[..at_index]);
    let return_token = header_tokens
        .last()
        .ok_or_else(|| "missing_return_type".to_string())?;
    let return_sort = parse_sort(return_token)?;
    let params_start = header[at_index..]
        .find('(')
        .map(|index| at_index + index + 1)
        .ok_or_else(|| "missing_param_list".to_string())?;
    let params_end = header[params_start..]
        .find(')')
        .map(|index| params_start + index)
        .ok_or_else(|| "unterminated_param_list".to_string())?;
    let params = parse_params(&header[params_start..params_end])?;

    let mut blocks = BTreeMap::<String, Block>::new();
    let mut current_label = None::<String>;
    let mut entry_label = None::<String>;
    let mut current_lines = Vec::<String>::new();
    for raw in lines {
        if raw.is_empty() || raw == "}" || raw.starts_with(';') || raw.starts_with("define ") {
            continue;
        }
        if raw.ends_with(':') {
            if let Some(label) = current_label.take() {
                blocks.insert(label.clone(), parse_block(&label, &current_lines)?);
                current_lines.clear();
            }
            let normalized = normalize_label(raw.trim_end_matches(':'));
            if entry_label.is_none() {
                entry_label = Some(normalized.clone());
            }
            current_label = Some(normalized);
            continue;
        }
        current_lines.push(raw.to_string());
    }
    if let Some(label) = current_label.take() {
        blocks.insert(label.clone(), parse_block(&label, &current_lines)?);
    }
    if blocks.is_empty() {
        return Err("no_basic_blocks".to_string());
    }

    Ok(Function {
        params,
        return_sort,
        entry_label: entry_label.ok_or_else(|| "missing_entry_block".to_string())?,
        blocks,
    })
}

fn parse_params(payload: &str) -> Result<Vec<Param>, String> {
    if payload.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut params = Vec::new();
    for (index, segment) in split_top_level(payload, ',').into_iter().enumerate() {
        let trimmed = segment.trim();
        if trimmed == "..." {
            return Err("variadic_functions_unsupported".to_string());
        }
        let tokens = split_ir_tokens_top_level(trimmed);
        let Some(type_index) = tokens.iter().position(|token| parse_sort(token).is_ok()) else {
            return Err(format!("unsupported_param_segment:{trimmed}"));
        };
        let sort = parse_sort(&tokens[type_index])?;
        let local_name = tokens
            .iter()
            .rev()
            .find(|token| token.starts_with('%'))
            .map_or_else(|| format!("%arg{index}"), |token| normalize_local(token));
        params.push(Param {
            canonical_name: format!("arg{index}"),
            local_name,
            sort,
        });
    }
    Ok(params)
}

fn parse_block(label: &str, lines: &[String]) -> Result<Block, String> {
    let mut phis = Vec::new();
    let mut instructions = Vec::new();
    let mut terminator = None::<Terminator>;
    for line in coalesce_block_lines(lines) {
        if line.is_empty() {
            continue;
        }
        if line == "unreachable" {
            terminator = Some(Terminator::Unreachable);
            continue;
        }
        if line.starts_with("ret ")
            || line.starts_with("br ")
            || line.starts_with("switch ")
            || line.starts_with("resume ")
            || is_invoke_instruction(&line)
        {
            terminator = Some(parse_terminator(&line)?);
            continue;
        }
        if let Some((left, right)) = line.split_once(" = ") {
            let dest = normalize_local(left.trim());
            if right.starts_with("phi ") {
                phis.push(parse_phi(&dest, right)?);
            } else if is_invoke_instruction(right) {
                terminator = Some(parse_invoke_terminator(right, Some(dest))?);
            } else {
                instructions.push(Instruction::Assign(Assign {
                    dest,
                    rhs: parse_rhs(right)?,
                }));
            }
            continue;
        }
        if line.starts_with("store ") {
            instructions.push(Instruction::Store(parse_store(&line)?));
            continue;
        }
        if is_call_instruction(&line) {
            instructions.push(Instruction::Call(parse_call_site(&line)?));
            continue;
        }
        return Err(format!("unsupported_instruction:{label}:{line}"));
    }

    Ok(Block {
        label: label.to_string(),
        phis,
        instructions,
        terminator: terminator.ok_or_else(|| format!("missing_terminator:{label}"))?,
    })
}

fn coalesce_block_lines(lines: &[String]) -> Vec<String> {
    let mut logical_lines = Vec::new();
    let mut index = 0_usize;
    while index < lines.len() {
        let line = strip_llvm_suffix(&lines[index]);
        if line.is_empty() {
            index += 1;
            continue;
        }
        if line.starts_with("switch ") {
            let mut combined = line;
            index += 1;
            while index < lines.len() {
                let next = strip_llvm_suffix(&lines[index]);
                if !next.is_empty() {
                    combined.push('\n');
                    combined.push_str(next.trim());
                }
                index += 1;
                if combined.contains('\n') && combined.contains(']') {
                    break;
                }
            }
            logical_lines.push(combined);
            continue;
        }
        if is_invoke_line(&line) && !line.contains(" unwind label ") {
            let mut combined = line;
            index += 1;
            while index < lines.len() {
                let next = strip_llvm_suffix(&lines[index]);
                index += 1;
                if next.is_empty() {
                    continue;
                }
                combined.push(' ');
                combined.push_str(next.trim());
                if combined.contains(" unwind label ") {
                    break;
                }
            }
            logical_lines.push(combined);
            continue;
        }
        if line.contains(" = landingpad ") {
            let mut combined = line;
            index += 1;
            while index < lines.len() {
                let next = strip_llvm_suffix(&lines[index]);
                if next.is_empty() {
                    index += 1;
                    continue;
                }
                if !is_landingpad_clause(&next) {
                    break;
                }
                combined.push(' ');
                combined.push_str(next.trim());
                index += 1;
            }
            logical_lines.push(combined);
            continue;
        }
        logical_lines.push(line);
        index += 1;
    }
    logical_lines
}

fn parse_phi(dest: &str, rhs: &str) -> Result<Phi, String> {
    let payload = rhs
        .strip_prefix("phi ")
        .ok_or_else(|| format!("invalid_phi:{rhs}"))?;
    let mut chars = payload.chars();
    let sort_token = take_type_token(&mut chars);
    let sort = parse_sort(&sort_token)?;
    let remainder = chars.as_str().trim();
    let mut incomings = Vec::new();
    for group in extract_bracket_groups(remainder)? {
        let (value_raw, pred_raw) = group
            .split_once(',')
            .ok_or_else(|| format!("invalid_phi_incoming:{group}"))?;
        incomings.push(PhiIncoming {
            predecessor: normalize_label(pred_raw.trim().trim_start_matches('%')),
            value: parse_value_ref(value_raw.trim(), &sort)?,
        });
    }
    if incomings.is_empty() {
        return Err("phi_without_incomings".to_string());
    }
    Ok(Phi {
        dest: dest.to_string(),
        incomings,
    })
}

fn parse_rhs(rhs: &str) -> Result<Rhs, String> {
    let trimmed = rhs.trim();
    if trimmed.starts_with("add ")
        || trimmed.starts_with("sub ")
        || trimmed.starts_with("mul ")
        || trimmed.starts_with("udiv ")
        || trimmed.starts_with("sdiv ")
        || trimmed.starts_with("urem ")
        || trimmed.starts_with("srem ")
        || trimmed.starts_with("and ")
        || trimmed.starts_with("or ")
        || trimmed.starts_with("xor ")
        || trimmed.starts_with("shl ")
        || trimmed.starts_with("lshr ")
        || trimmed.starts_with("ashr ")
    {
        return parse_binary_rhs(trimmed);
    }
    if trimmed.starts_with("icmp ") {
        return parse_compare_rhs(trimmed);
    }
    if trimmed.starts_with("select ") {
        return parse_select_rhs(trimmed);
    }
    if trimmed.starts_with("zext ")
        || trimmed.starts_with("sext ")
        || trimmed.starts_with("trunc ")
        || trimmed.starts_with("bitcast ")
    {
        return parse_cast_rhs(trimmed);
    }
    if trimmed.starts_with("load ") {
        return parse_load_rhs(trimmed);
    }
    if trimmed.starts_with("getelementptr ") {
        return parse_gep_rhs(trimmed);
    }
    if is_call_instruction(trimmed) {
        return Ok(Rhs::Call(parse_call_site(trimmed)?));
    }
    if trimmed.starts_with("extractvalue ") {
        return parse_extractvalue_rhs(trimmed);
    }
    if trimmed.starts_with("landingpad ") {
        return parse_landingpad_rhs(trimmed);
    }
    if trimmed.starts_with("alloca ") {
        return parse_alloca_rhs(trimmed);
    }
    if trimmed.starts_with("invoke ") || trimmed.starts_with("switch ") {
        return Err(format!("unsupported_semantic_instruction:{trimmed}"));
    }
    if let Some((sort, value)) = parse_typed_value(trimmed)? {
        return Ok(Rhs::Value { sort, value });
    }
    Err(format!("unsupported_rhs:{trimmed}"))
}

fn parse_binary_rhs(rhs: &str) -> Result<Rhs, String> {
    let opcode = rhs
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("missing_binary_opcode:{rhs}"))?;
    let op = parse_bin_op(opcode)?;
    let tokens = split_ir_tokens_top_level(rhs);
    let Some(type_index) = tokens.iter().position(|token| parse_sort(token).is_ok()) else {
        return Err(format!("missing_binary_type:{rhs}"));
    };
    let sort = parse_sort(&tokens[type_index])?;
    let payload = tokens[type_index + 1..].join(" ");
    let (left_raw, right_raw) = payload
        .split_once(',')
        .ok_or_else(|| format!("invalid_binary_operands:{rhs}"))?;
    Ok(Rhs::Binary {
        op,
        sort: sort.clone(),
        left: parse_value_ref(left_raw.trim(), &sort)?,
        right: parse_value_ref(right_raw.trim(), &sort)?,
    })
}

fn parse_compare_rhs(rhs: &str) -> Result<Rhs, String> {
    let tokens = split_ir_tokens_top_level(rhs);
    if tokens.len() < 5 {
        return Err(format!("invalid_icmp:{rhs}"));
    }
    let operand_sort = parse_sort(&tokens[2])?;
    let payload = tokens[3..].join(" ");
    let (left_raw, right_raw) = payload
        .split_once(',')
        .ok_or_else(|| format!("invalid_icmp_operands:{rhs}"))?;
    Ok(Rhs::Compare {
        pred: parse_cmp_pred(&tokens[1])?,
        operand_sort: operand_sort.clone(),
        left: parse_value_ref(left_raw.trim(), &operand_sort)?,
        right: parse_value_ref(right_raw.trim(), &operand_sort)?,
    })
}

fn parse_select_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("select ")
        .ok_or_else(|| format!("invalid_select:{rhs}"))?;
    let segments = split_top_level(payload, ',');
    if segments.len() != 3 {
        return Err(format!("invalid_select_segments:{rhs}"));
    }
    let Some((cond_sort, cond)) = parse_typed_value(&segments[0])? else {
        return Err(format!("invalid_select_condition:{rhs}"));
    };
    if cond_sort != Sort::Bool {
        return Err(format!("select_condition_not_bool:{rhs}"));
    }
    let Some((sort, then_value)) = parse_typed_value(&segments[1])? else {
        return Err(format!("invalid_select_then:{rhs}"));
    };
    let Some((else_sort, else_value)) = parse_typed_value(&segments[2])? else {
        return Err(format!("invalid_select_else:{rhs}"));
    };
    if sort != else_sort {
        return Err(format!("select_type_mismatch:{rhs}"));
    }
    Ok(Rhs::Select {
        sort,
        cond,
        then_value,
        else_value,
    })
}

fn parse_cast_rhs(rhs: &str) -> Result<Rhs, String> {
    let opcode = rhs
        .split_whitespace()
        .next()
        .ok_or_else(|| format!("missing_cast_opcode:{rhs}"))?;
    let kind = match opcode {
        "zext" => CastKind::ZExt,
        "sext" => CastKind::SExt,
        "trunc" => CastKind::Trunc,
        "bitcast" => CastKind::BitCast,
        _ => return Err(format!("unsupported_cast_opcode:{rhs}")),
    };
    let payload = rhs[opcode.len()..].trim();
    let (left, right) = payload
        .split_once(" to ")
        .ok_or_else(|| format!("invalid_cast_rhs:{rhs}"))?;
    let Some((from, value)) = parse_typed_value(left)? else {
        return Err(format!("invalid_cast_source:{rhs}"));
    };
    Ok(Rhs::Cast {
        kind,
        from,
        to: parse_sort(right.trim())?,
        value,
    })
}

fn parse_load_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("load ")
        .ok_or_else(|| format!("invalid_load:{rhs}"))?;
    let segments = split_top_level(payload, ',');
    if segments.len() < 2 {
        return Err(format!("invalid_load_segments:{rhs}"));
    }
    let sort = parse_last_sort_token(&segments[0])?;
    let Some((pointer_sort, pointer)) = parse_typed_value(&segments[1])? else {
        return Err(format!("invalid_load_pointer:{rhs}"));
    };
    if pointer_sort != Sort::Ptr {
        return Err(format!("load_pointer_not_ptr:{rhs}"));
    }
    Ok(Rhs::Load { sort, pointer })
}

fn parse_gep_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("getelementptr ")
        .ok_or_else(|| format!("invalid_gep:{rhs}"))?;
    let segments = split_top_level(payload, ',');
    if segments.len() < 2 {
        return Err(format!("invalid_gep_segments:{rhs}"));
    }
    let source_element = parse_last_layout_token(&segments[0])?;
    let Some((pointer_sort, base)) = parse_typed_value(&segments[1])? else {
        return Err(format!("invalid_gep_base:{rhs}"));
    };
    if pointer_sort != Sort::Ptr {
        return Err(format!("gep_base_not_ptr:{rhs}"));
    }
    let mut indices = Vec::new();
    for segment in segments.iter().skip(2) {
        indices.push(parse_gep_index(segment)?);
    }
    if indices.is_empty() {
        return Err(format!("gep_without_indices:{rhs}"));
    }
    Ok(Rhs::GetElementPtr {
        source_element,
        base,
        indices,
    })
}

fn parse_extractvalue_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("extractvalue ")
        .ok_or_else(|| format!("invalid_extractvalue:{rhs}"))?;
    let segments = split_top_level(payload, ',');
    if segments.len() < 2 {
        return Err(format!("invalid_extractvalue_segments:{rhs}"));
    }
    let Some((aggregate_sort, aggregate)) = parse_typed_value(&segments[0])? else {
        return Err(format!("invalid_extractvalue_aggregate:{rhs}"));
    };
    if !matches!(aggregate_sort, Sort::Aggregate(_)) {
        return Err(format!("extractvalue_non_aggregate:{rhs}"));
    }
    let mut indices = Vec::new();
    for segment in segments.iter().skip(1) {
        let index = segment
            .trim()
            .parse::<usize>()
            .map_err(|_| format!("invalid_extractvalue_index:{rhs}"))?;
        indices.push(index);
    }
    Ok(Rhs::ExtractValue { aggregate, indices })
}

fn parse_alloca_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("alloca ")
        .ok_or_else(|| format!("invalid_alloca:{rhs}"))?;
    let segments = split_top_level(payload, ',');
    let Some(first) = segments.first() else {
        return Err(format!("invalid_alloca_segments:{rhs}"));
    };
    Ok(Rhs::Alloca {
        allocated: parse_last_layout_token(first)?,
    })
}

fn parse_landingpad_rhs(rhs: &str) -> Result<Rhs, String> {
    let payload = rhs
        .strip_prefix("landingpad ")
        .ok_or_else(|| format!("invalid_landingpad:{rhs}"))?;
    let mut chars = payload.chars();
    let sort_token = take_type_token(&mut chars);
    Ok(Rhs::LandingPad {
        sort: parse_sort(&sort_token)?,
    })
}

fn parse_store(line: &str) -> Result<Store, String> {
    let payload = line
        .strip_prefix("store ")
        .ok_or_else(|| format!("invalid_store:{line}"))?;
    let segments = split_top_level(payload, ',');
    if segments.len() < 2 {
        return Err(format!("invalid_store_segments:{line}"));
    }
    let Some((value_sort, value)) = parse_typed_value(&segments[0])? else {
        return Err(format!("invalid_store_value:{line}"));
    };
    let Some((pointer_sort, pointer)) = parse_typed_value(&segments[1])? else {
        return Err(format!("invalid_store_pointer:{line}"));
    };
    if pointer_sort != Sort::Ptr {
        return Err(format!("store_pointer_not_ptr:{line}"));
    }
    Ok(Store {
        value_sort,
        value,
        pointer,
    })
}

fn parse_call_site(line: &str) -> Result<CallSite, String> {
    let payload = strip_call_prefix(line).ok_or_else(|| format!("invalid_call:{line}"))?;
    parse_call_site_payload(payload, line)
}

fn parse_call_site_payload(payload: &str, context: &str) -> Result<CallSite, String> {
    let open_paren = payload
        .find('(')
        .ok_or_else(|| format!("missing_call_args:{context}"))?;
    let close_paren = find_matching_paren(payload, open_paren)
        .ok_or_else(|| format!("unterminated_call_args:{context}"))?;
    let prefix = payload[..open_paren].trim();
    let args_payload = payload[open_paren + 1..close_paren].trim();
    let prefix_tokens = split_ir_tokens_top_level(prefix);
    let Some(callee_index) = prefix_tokens
        .iter()
        .rposition(|token| token.starts_with('@') || token.starts_with('%'))
    else {
        return Err(format!("missing_call_callee:{context}"));
    };
    if callee_index == 0 {
        return Err(format!("missing_call_return_type:{context}"));
    }
    let return_sort = parse_sort(&prefix_tokens[callee_index - 1])?;
    let callee = prefix_tokens[callee_index].clone();
    let mut args = Vec::new();
    if !args_payload.is_empty() {
        for segment in split_top_level(args_payload, ',') {
            if segment.trim() == "..." {
                return Err(format!("variadic_call_unsupported:{context}"));
            }
            let Some((sort, value)) = parse_typed_value(&segment)? else {
                return Err(format!("invalid_call_argument:{context}:{segment}"));
            };
            args.push(TypedValue { sort, value });
        }
    }
    Ok(CallSite {
        callee,
        return_sort,
        args,
    })
}

fn parse_typed_value(segment: &str) -> Result<Option<(Sort, ValueRef)>, String> {
    let trimmed = segment.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let tokens = split_ir_tokens_top_level(trimmed);
    let Some(type_index) = tokens.iter().position(|token| parse_sort(token).is_ok()) else {
        return Ok(None);
    };
    let sort = parse_sort(&tokens[type_index])?;
    let value_token = extract_value_token(&tokens[type_index + 1..])
        .ok_or_else(|| format!("missing_typed_value:{segment}"))?;
    Ok(Some((sort.clone(), parse_value_ref(&value_token, &sort)?)))
}

fn parse_gep_index(segment: &str) -> Result<i64, String> {
    let Some((sort, value)) = parse_typed_value(segment)? else {
        return Err(format!("invalid_gep_index:{segment}"));
    };
    let Sort::BitVec(width) = sort else {
        return Err(format!("non_integer_gep_index:{segment}"));
    };
    let ValueRef::BitVecConst { .. } = value else {
        return Err(format!("dynamic_gep_index_unsupported:{segment}"));
    };
    let raw_token = extract_value_token(&split_ir_tokens_top_level(segment)[1..])
        .ok_or_else(|| format!("invalid_gep_index:{segment}"))?;
    let signed = raw_token
        .parse::<i64>()
        .map_err(|_| format!("unsupported_gep_index:{segment}"))?;
    if width == 0 {
        return Err(format!("unsupported_gep_index_width:{segment}"));
    }
    Ok(signed)
}

fn parse_terminator(line: &str) -> Result<Terminator, String> {
    if let Some(payload) = line.strip_prefix("ret ") {
        let trimmed = payload.trim();
        if trimmed == "void" {
            return Ok(Terminator::Ret { value: None });
        }
        let Some((sort, value)) = parse_typed_value(trimmed)? else {
            return Err(format!("invalid_return_value:{line}"));
        };
        if matches!(sort, Sort::Void) {
            return Err("void_return_value".to_string());
        }
        return Ok(Terminator::Ret { value: Some(value) });
    }
    if let Some(payload) = line.strip_prefix("br label ") {
        return Ok(Terminator::Br {
            destination: normalize_label(payload.trim().trim_start_matches('%')),
        });
    }
    if let Some(payload) = line.strip_prefix("br ") {
        let segments = split_top_level(payload, ',');
        if segments.len() != 3 {
            return Err(format!("invalid_conditional_branch:{line}"));
        }
        let Some((cond_sort, cond)) = parse_typed_value(&segments[0])? else {
            return Err(format!("invalid_branch_condition:{line}"));
        };
        if cond_sort != Sort::Bool {
            return Err(format!("branch_condition_not_bool:{line}"));
        }
        return Ok(Terminator::CondBr {
            cond,
            then_label: normalize_label(
                segments[1]
                    .trim()
                    .strip_prefix("label ")
                    .ok_or_else(|| format!("invalid_then_label:{line}"))?
                    .trim()
                    .trim_start_matches('%'),
            ),
            else_label: normalize_label(
                segments[2]
                    .trim()
                    .strip_prefix("label ")
                    .ok_or_else(|| format!("invalid_else_label:{line}"))?
                    .trim()
                    .trim_start_matches('%'),
            ),
        });
    }
    if line.starts_with("switch ") {
        return parse_switch_terminator(line);
    }
    if is_invoke_instruction(line) {
        return parse_invoke_terminator(line, None);
    }
    if let Some(payload) = line.strip_prefix("resume ") {
        let trimmed = payload.trim();
        let Some((_sort, _value)) = parse_typed_value(trimmed)? else {
            return Err(format!("invalid_resume_value:{line}"));
        };
        return Ok(Terminator::Resume);
    }
    Err(format!("unsupported_terminator:{line}"))
}

fn parse_switch_terminator(line: &str) -> Result<Terminator, String> {
    let payload = line
        .strip_prefix("switch ")
        .ok_or_else(|| format!("invalid_switch:{line}"))?;
    let bracket_start = payload
        .find('[')
        .ok_or_else(|| format!("missing_switch_cases:{line}"))?;
    let bracket_end = payload
        .rfind(']')
        .ok_or_else(|| format!("unterminated_switch_cases:{line}"))?;
    let head = payload[..bracket_start].trim();
    let case_payload = payload[bracket_start + 1..bracket_end].trim();
    let Some((operand_sort, discriminator)) = parse_typed_value(
        head.split_once(", label ")
            .map(|(value, _)| value)
            .ok_or_else(|| format!("invalid_switch_head:{line}"))?,
    )?
    else {
        return Err(format!("invalid_switch_discriminator:{line}"));
    };
    let default_label = normalize_label(
        head.split_once(", label ")
            .map(|(_, label)| label)
            .ok_or_else(|| format!("invalid_switch_default:{line}"))?
            .trim()
            .trim_start_matches('%'),
    );
    let mut cases = Vec::new();
    for raw_case in case_payload
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let (value_raw, destination_raw) = raw_case
            .split_once(", label ")
            .ok_or_else(|| format!("invalid_switch_case:{raw_case}"))?;
        let Some((case_sort, value)) = parse_typed_value(value_raw)? else {
            return Err(format!("invalid_switch_case_value:{raw_case}"));
        };
        if case_sort != operand_sort {
            return Err(format!("switch_case_sort_mismatch:{raw_case}"));
        }
        cases.push(SwitchCase {
            value,
            destination: normalize_label(destination_raw.trim().trim_start_matches('%')),
        });
    }
    if cases.is_empty() && !case_payload.is_empty() {
        return Err(format!("invalid_switch_cases:{line}"));
    }
    Ok(Terminator::Switch {
        operand_sort,
        discriminator,
        default_label,
        cases,
    })
}

fn parse_invoke_terminator(line: &str, dest: Option<String>) -> Result<Terminator, String> {
    let payload = strip_invoke_prefix(line).ok_or_else(|| format!("invalid_invoke:{line}"))?;
    let (call_payload, edge_payload) = payload
        .split_once(" to label ")
        .ok_or_else(|| format!("invalid_invoke_edges:{line}"))?;
    let (normal_label, unwind_label) = edge_payload
        .split_once(" unwind label ")
        .ok_or_else(|| format!("invalid_invoke_unwind:{line}"))?;
    Ok(Terminator::Invoke(InvokeTerminator {
        dest,
        call: parse_call_site_payload(call_payload.trim(), line)?,
        normal_label: normalize_label(normal_label.trim().trim_start_matches('%')),
        unwind_label: normalize_label(unwind_label.trim().trim_start_matches('%')),
    }))
}

fn parse_sort(token: &str) -> Result<Sort, String> {
    let trimmed = token.trim().trim_end_matches(',');
    if trimmed == "void" {
        return Ok(Sort::Void);
    }
    if trimmed == "ptr" {
        return Ok(Sort::Ptr);
    }
    if trimmed == "i1" {
        return Ok(Sort::Bool);
    }
    if let Some(width) = trimmed.strip_prefix('i') {
        let width = width
            .parse::<u16>()
            .map_err(|_| format!("invalid_integer_sort:{trimmed}"))?;
        if width == 0 || width > 128 {
            return Err(format!("unsupported_integer_width:{trimmed}"));
        }
        return Ok(Sort::BitVec(width));
    }
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let inner = trimmed[1..trimmed.len() - 1].trim();
        if inner.is_empty() {
            return Err("empty_aggregate_sort".to_string());
        }
        return Ok(Sort::Aggregate(
            split_top_level(inner, ',')
                .into_iter()
                .map(|segment| parse_sort(&segment))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    Err(format!("unsupported_sort:{trimmed}"))
}

fn parse_layout_type(token: &str) -> Result<LayoutType, String> {
    let trimmed = token.trim().trim_end_matches(',');
    if trimmed == "void" {
        return Ok(LayoutType::Void);
    }
    if trimmed == "ptr" {
        return Ok(LayoutType::Ptr);
    }
    if trimmed == "i1" {
        return Ok(LayoutType::Int(1));
    }
    if let Some(width) = trimmed.strip_prefix('i') {
        let width = width
            .parse::<u16>()
            .map_err(|_| format!("invalid_layout_integer:{trimmed}"))?;
        if width == 0 || width > 128 {
            return Err(format!("unsupported_layout_integer:{trimmed}"));
        }
        return Ok(LayoutType::Int(width));
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        let inner = trimmed[1..trimmed.len() - 1].trim();
        let Some((count_raw, element_raw)) = inner.split_once('x') else {
            return Err(format!("invalid_array_layout:{trimmed}"));
        };
        let count = count_raw
            .trim()
            .parse::<u64>()
            .map_err(|_| format!("invalid_array_layout:{trimmed}"))?;
        return Ok(LayoutType::Array(
            count,
            Box::new(parse_layout_type(element_raw.trim())?),
        ));
    }
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        let inner = trimmed[1..trimmed.len() - 1].trim();
        return Ok(LayoutType::Struct(
            split_top_level(inner, ',')
                .into_iter()
                .map(|segment| parse_layout_type(&segment))
                .collect::<Result<Vec<_>, _>>()?,
        ));
    }
    Err(format!("unsupported_layout_type:{trimmed}"))
}

fn parse_last_sort_token(segment: &str) -> Result<Sort, String> {
    split_ir_tokens_top_level(segment)
        .into_iter()
        .rev()
        .find_map(|token| parse_sort(&token).ok())
        .ok_or_else(|| format!("missing_sort_token:{segment}"))
}

fn parse_last_layout_token(segment: &str) -> Result<LayoutType, String> {
    split_ir_tokens_top_level(segment)
        .into_iter()
        .rev()
        .find_map(|token| parse_layout_type(&token).ok())
        .ok_or_else(|| format!("missing_layout_token:{segment}"))
}

fn parse_bin_op(opcode: &str) -> Result<BinOp, String> {
    match opcode {
        "add" => Ok(BinOp::Add),
        "sub" => Ok(BinOp::Sub),
        "mul" => Ok(BinOp::Mul),
        "udiv" => Ok(BinOp::UDiv),
        "sdiv" => Ok(BinOp::SDiv),
        "urem" => Ok(BinOp::URem),
        "srem" => Ok(BinOp::SRem),
        "and" => Ok(BinOp::And),
        "or" => Ok(BinOp::Or),
        "xor" => Ok(BinOp::Xor),
        "shl" => Ok(BinOp::Shl),
        "lshr" => Ok(BinOp::LShr),
        "ashr" => Ok(BinOp::AShr),
        _ => Err(format!("unsupported_binary_opcode:{opcode}")),
    }
}

fn parse_cmp_pred(predicate: &str) -> Result<CmpPred, String> {
    match predicate {
        "eq" => Ok(CmpPred::Eq),
        "ne" => Ok(CmpPred::Ne),
        "ult" => Ok(CmpPred::Ult),
        "ule" => Ok(CmpPred::Ule),
        "ugt" => Ok(CmpPred::Ugt),
        "uge" => Ok(CmpPred::Uge),
        "slt" => Ok(CmpPred::Slt),
        "sle" => Ok(CmpPred::Sle),
        "sgt" => Ok(CmpPred::Sgt),
        "sge" => Ok(CmpPred::Sge),
        _ => Err(format!("unsupported_icmp_predicate:{predicate}")),
    }
}

fn parse_value_ref(token: &str, sort: &Sort) -> Result<ValueRef, String> {
    let trimmed = token.trim().trim_end_matches(',');
    if trimmed.starts_with('%') {
        return Ok(ValueRef::Local(normalize_local(trimmed)));
    }
    if trimmed.starts_with('@') {
        return Ok(ValueRef::Global(trimmed.to_string()));
    }
    match sort {
        Sort::Bool => match trimmed {
            "true" | "1" => Ok(ValueRef::BoolConst(true)),
            "false" | "0" => Ok(ValueRef::BoolConst(false)),
            _ => Err(format!("invalid_bool_value:{trimmed}")),
        },
        Sort::BitVec(width) => Ok(ValueRef::BitVecConst {
            width: *width,
            value: parse_bitvec_constant(trimmed, *width)?,
        }),
        Sort::Ptr => match trimmed {
            "null" => Ok(ValueRef::Null),
            _ => Err(format!("invalid_ptr_value:{trimmed}")),
        },
        Sort::Aggregate(_) => Err(format!("aggregate_constant_unsupported:{trimmed}")),
        Sort::Void => Err("void_value_reference".to_string()),
    }
}

fn parse_bitvec_constant(token: &str, width: u16) -> Result<u128, String> {
    let parsed = token
        .parse::<i128>()
        .map_err(|_| format!("unsupported_bitvec_constant:{token}"))?;
    let raw = parsed.cast_unsigned();
    Ok(if width >= 128 {
        raw
    } else {
        raw % (1_u128 << u32::from(width))
    })
}

fn strip_llvm_suffix(line: &str) -> String {
    let without_comment = line.split(';').next().unwrap_or_default().trim();
    if let Some((prefix, _)) = without_comment.split_once(", !") {
        return prefix.trim().to_string();
    }
    without_comment.to_string()
}

fn normalize_local(token: &str) -> String {
    if token.starts_with('%') {
        token.to_string()
    } else {
        format!("%{token}")
    }
}

fn normalize_label(token: &str) -> String {
    token.trim().trim_start_matches('%').to_string()
}

fn take_type_token(chars: &mut std::str::Chars<'_>) -> String {
    let mut token = String::new();
    let mut brace_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    for ch in chars.by_ref() {
        match ch {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            _ => {}
        }
        if ch.is_whitespace() && brace_depth == 0 && bracket_depth == 0 {
            if !token.is_empty() {
                break;
            }
            continue;
        }
        token.push(ch);
        if brace_depth == 0 && bracket_depth == 0 && ch == '}' {
            break;
        }
    }
    token
}

fn split_top_level(payload: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    let mut brace_depth = 0_i32;
    for ch in payload.chars() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            _ => {}
        }
        if ch == separator && paren_depth == 0 && bracket_depth == 0 && brace_depth == 0 {
            parts.push(current.trim().to_string());
            current.clear();
            continue;
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn split_ir_tokens_top_level(payload: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut paren_depth = 0_i32;
    let mut bracket_depth = 0_i32;
    let mut brace_depth = 0_i32;
    let mut in_quotes = false;
    for ch in payload.chars() {
        match ch {
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => paren_depth += 1,
            ')' if !in_quotes => paren_depth -= 1,
            '[' if !in_quotes => bracket_depth += 1,
            ']' if !in_quotes => bracket_depth -= 1,
            '{' if !in_quotes => brace_depth += 1,
            '}' if !in_quotes => brace_depth -= 1,
            _ => {}
        }
        if ch.is_whitespace()
            && !in_quotes
            && paren_depth == 0
            && bracket_depth == 0
            && brace_depth == 0
        {
            if !current.is_empty() {
                tokens.push(current.trim().to_string());
                current.clear();
            }
            continue;
        }
        current.push(ch);
    }
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }
    tokens
}

fn extract_bracket_groups(payload: &str) -> Result<Vec<String>, String> {
    let mut groups = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    for ch in payload.chars() {
        if ch == '[' {
            if depth == 0 {
                current.clear();
            } else {
                current.push(ch);
            }
            depth += 1;
            continue;
        }
        if ch == ']' {
            depth -= 1;
            if depth < 0 {
                return Err("unbalanced_phi_brackets".to_string());
            }
            if depth == 0 {
                groups.push(current.trim().to_string());
                current.clear();
            } else {
                current.push(ch);
            }
            continue;
        }
        if depth > 0 {
            current.push(ch);
        }
    }
    if depth != 0 {
        return Err("unterminated_phi_brackets".to_string());
    }
    Ok(groups)
}

fn is_call_instruction(line: &str) -> bool {
    matches!(
        line,
        value if value.starts_with("call ")
            || value.starts_with("tail call ")
            || value.starts_with("musttail call ")
            || value.starts_with("notail call ")
    )
}

fn is_invoke_instruction(line: &str) -> bool {
    matches!(
        line,
        value if value.starts_with("invoke ")
            || value.starts_with("tail invoke ")
            || value.starts_with("musttail invoke ")
            || value.starts_with("notail invoke ")
    )
}

fn is_invoke_line(line: &str) -> bool {
    is_invoke_instruction(line)
        || line
            .split_once(" = ")
            .is_some_and(|(_, rhs)| is_invoke_instruction(rhs.trim()))
}

fn is_landingpad_clause(line: &str) -> bool {
    line == "cleanup" || line.starts_with("catch ") || line.starts_with("filter ")
}

fn strip_invoke_prefix(line: &str) -> Option<&str> {
    line.strip_prefix("tail invoke ")
        .or_else(|| line.strip_prefix("musttail invoke "))
        .or_else(|| line.strip_prefix("notail invoke "))
        .or_else(|| line.strip_prefix("invoke "))
}

fn strip_call_prefix(line: &str) -> Option<&str> {
    line.strip_prefix("tail call ")
        .or_else(|| line.strip_prefix("musttail call "))
        .or_else(|| line.strip_prefix("notail call "))
        .or_else(|| line.strip_prefix("call "))
}

fn find_matching_paren(payload: &str, open_index: usize) -> Option<usize> {
    let mut depth = 0_i32;
    let mut in_quotes = false;
    for (index, ch) in payload.char_indices().skip(open_index) {
        if ch == '"' {
            in_quotes = !in_quotes;
        }
        if in_quotes {
            continue;
        }
        if ch == '(' {
            depth += 1;
        } else if ch == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(index);
            }
        }
    }
    None
}

fn extract_value_token(tokens: &[String]) -> Option<String> {
    let mut index = 0_usize;
    while index < tokens.len() {
        let token = tokens[index].trim();
        if token.is_empty() || token.starts_with('#') || token == "..." {
            index += 1;
            continue;
        }
        if is_ir_qualifier(token) {
            if token == "align" && index + 1 < tokens.len() {
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        return Some(token.to_string());
    }
    None
}

fn is_ir_qualifier(token: &str) -> bool {
    matches!(
        token,
        "noundef"
            | "nonnull"
            | "noalias"
            | "nocapture"
            | "readonly"
            | "readnone"
            | "writeonly"
            | "signext"
            | "zeroext"
            | "returned"
            | "nest"
            | "swiftself"
            | "swifterror"
            | "align"
    ) || token.starts_with("align")
        || token.starts_with("dereferenceable")
        || token.starts_with("byval")
        || token.starts_with("sret")
        || token.starts_with("range")
        || token.starts_with("nonnull")
}
