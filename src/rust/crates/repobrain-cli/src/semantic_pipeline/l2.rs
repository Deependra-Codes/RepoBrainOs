use super::{
    BTreeMap, BTreeSet, CliL2AlignmentMode, Duration, EquivalenceEvidenceReceipt,
    EquivalenceEvidenceStatus, EquivalenceStage, Instant, L2AlignedPair, L2AlignmentResult,
    L2BehaviorProfile, L2FunctionSignal, L2RelationalOutcome, L2RelationalPolicy, Path,
    RepositoryInventorySnapshot, SemanticDelta, SymbolKind, semantic_diff_summary,
    symbol_evidence_label, symbol_kind_label,
};

pub(crate) fn l2_relational_semantic_receipt(
    reference_snapshot: &RepositoryInventorySnapshot,
    target_snapshot: &RepositoryInventorySnapshot,
    delta: &SemanticDelta,
    policy: L2RelationalPolicy,
) -> EquivalenceEvidenceReceipt {
    let mut assumptions = l2_base_assumptions();
    let bounds = l2_bounds(policy);

    if !delta.has_changes() {
        assumptions.push("L0 observed no structural delta, so L2 reports no relational difference under empty scope".to_string());
        return l2_receipt(
            EquivalenceEvidenceStatus::NoDifferenceObserved,
            bounds,
            policy.timeout_ms,
            assumptions,
            None,
        );
    }

    let changed_paths = l2_changed_paths(delta);
    assumptions.push(format!(
        "L2 scope covers {} changed path(s) from structural delta evidence",
        changed_paths.len()
    ));
    let deadline = Instant::now() + Duration::from_millis(policy.timeout_ms);
    let reference_signals = l2_function_signals(
        reference_snapshot,
        &changed_paths,
        policy.max_candidate_functions,
    );
    let target_signals = l2_function_signals(
        target_snapshot,
        &changed_paths,
        policy.max_candidate_functions,
    );
    assumptions.push(format!(
        "candidate function scope selected {} reference and {} target function signal(s)",
        reference_signals.len(),
        target_signals.len()
    ));

    if reference_signals.is_empty() && target_signals.is_empty() {
        assumptions.push(
            "no function-level candidates were available; structural delta may be outside function semantics"
                .to_string(),
        );
        return l2_receipt(
            EquivalenceEvidenceStatus::Inconclusive,
            bounds,
            policy.timeout_ms,
            assumptions,
            semantic_delta_witness(delta),
        );
    }

    let alignment = l2_align_functions(&reference_signals, &target_signals, policy, deadline);
    assumptions.push(format!(
        "alignment produced {} pair(s), {} unmatched reference function(s), {} unmatched target function(s)",
        alignment.pairs.len(),
        alignment.unmatched_reference.len(),
        alignment.unmatched_target.len()
    ));
    if alignment.timed_out {
        assumptions.push(format!(
            "alignment/comparison exceeded timeout policy ({}ms) and returned bounded partial evidence",
            policy.timeout_ms
        ));
    }

    let outcome = l2_relational_outcome(&alignment, delta);
    let status = l2_status_from_outcome(&outcome);
    l2_receipt(
        status,
        bounds,
        policy.timeout_ms,
        assumptions,
        outcome.witness.or_else(|| semantic_delta_witness(delta)),
    )
}

fn l2_base_assumptions() -> Vec<String> {
    vec![
        "L2 relational semantic stage aligns changed function symbols using deterministic matching tiers".to_string(),
        "L2 builds normalized declaration/call/control/literal/window-hash behavior profiles from bounded source windows".to_string(),
        "L2 comparisons are bounded by candidate/pair/time policies and can be inconclusive".to_string(),
        "current L2 evidence compares snapshot-derived behavioral signatures; it is not a full theorem-proving equivalence proof".to_string(),
    ]
}

fn l2_bounds(policy: L2RelationalPolicy) -> String {
    format!(
        "pairs<={};candidate_functions<={};alignment_mode={};min_alignment_score>={};profile=decl+call+control+literal+window_hash",
        policy.max_pairs,
        policy.max_candidate_functions,
        l2_alignment_mode_label(policy.alignment_mode),
        policy.min_alignment_score
    )
}

fn l2_alignment_mode_label(mode: CliL2AlignmentMode) -> &'static str {
    match mode {
        CliL2AlignmentMode::ExactOnly => "exact_only",
        CliL2AlignmentMode::SignatureAware => "signature_aware",
    }
}

pub(super) fn l2_changed_paths(delta: &SemanticDelta) -> BTreeSet<String> {
    let mut changed_paths = BTreeSet::new();
    changed_paths.extend(delta.changed_scope.iter().cloned());
    changed_paths.extend(delta.modified_files.iter().cloned());
    changed_paths.extend(delta.added_files.iter().cloned());
    changed_paths.extend(delta.removed_files.iter().cloned());
    changed_paths
}

fn l2_manifest_candidates(snapshot: &RepositoryInventorySnapshot) -> BTreeSet<String> {
    snapshot
        .files
        .iter()
        .filter(|file| file.relative_path.ends_with("Cargo.toml"))
        .map(|file| file.relative_path.replace('\\', "/"))
        .collect()
}

fn l2_manifest_path_for_source_path(
    manifest_candidates: &BTreeSet<String>,
    source_path: &str,
) -> Option<String> {
    let mut cursor = Path::new(source_path).parent();
    while let Some(directory) = cursor {
        let candidate = directory.join("Cargo.toml");
        let candidate = candidate.to_string_lossy().replace('\\', "/");
        if manifest_candidates.contains(&candidate) {
            return Some(candidate);
        }
        cursor = directory.parent();
    }

    if manifest_candidates.contains("Cargo.toml") {
        return Some("Cargo.toml".to_string());
    }

    None
}

pub(super) fn l2_function_signals(
    snapshot: &RepositoryInventorySnapshot,
    changed_paths: &BTreeSet<String>,
    max_candidate_functions: usize,
) -> Vec<L2FunctionSignal> {
    let manifest_candidates = l2_manifest_candidates(snapshot);
    let import_context = l2_import_context_signatures(snapshot);
    let reverse_import_context = l2_reverse_import_context_signatures(snapshot);
    let symbol_context = l2_symbol_context_signatures(snapshot);
    let function_neighbors = l2_function_neighbor_signatures(snapshot);
    let mut symbols = snapshot
        .symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Function))
        .filter(|symbol| changed_paths.contains(&symbol.relative_path))
        .collect::<Vec<_>>();
    symbols.sort_unstable_by(|left, right| {
        left.relative_path
            .cmp(&right.relative_path)
            .then_with(|| left.line_number.cmp(&right.line_number))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.fact_id.cmp(&right.fact_id))
    });
    let mut signals = Vec::new();
    for symbol in symbols.into_iter().take(max_candidate_functions) {
        let import_signature = import_context
            .get(&symbol.relative_path)
            .cloned()
            .unwrap_or_default();
        let reverse_import_signature = reverse_import_context
            .get(&symbol.relative_path)
            .cloned()
            .unwrap_or_default();
        let symbol_context_signature = symbol_context
            .get(&symbol.relative_path)
            .cloned()
            .unwrap_or_default();
        let neighbor_signature = function_neighbors
            .get(&symbol.fact_id)
            .cloned()
            .unwrap_or_default();
        let profile = l2_behavior_profile_for_symbol(
            symbol,
            &import_signature,
            &symbol_context_signature,
            &neighbor_signature,
        );
        let name_key = normalize_identifier(&symbol.name);
        let behavior_signature = format!(
            "lang={}|decl={}|calls={}|control={}|literals={}|window_hash={}|imports={}|symbol_context={}|evidence={}",
            symbol.language,
            profile.declaration_signature,
            profile.call_signature,
            profile.control_signature,
            profile.literal_signature,
            profile.normalized_window_hash,
            import_signature,
            symbol_context_signature,
            symbol_evidence_label(symbol.evidence_class)
        );

        signals.push(L2FunctionSignal {
            id: format!(
                "{}@{}:{}",
                symbol.name, symbol.relative_path, symbol.line_number
            ),
            name: symbol.name.clone(),
            name_key,
            relative_path: symbol.relative_path.clone(),
            manifest_path: l2_manifest_path_for_source_path(
                &manifest_candidates,
                &symbol.relative_path,
            ),
            language: symbol.language.clone(),
            line_number: symbol.line_number,
            import_signature,
            reverse_import_signature,
            symbol_context_signature,
            behavior_profile: profile,
            behavior_signature,
        });
    }

    signals
}

fn l2_reverse_import_context_signatures(
    snapshot: &RepositoryInventorySnapshot,
) -> BTreeMap<String, String> {
    const MAX_CONTEXT_ITEMS: usize = 24;

    let mut per_path = BTreeMap::<String, BTreeSet<String>>::new();
    for import in &snapshot.imports {
        let Some(resolved_path) = import.resolved_path.as_deref() else {
            continue;
        };
        let _ = per_path
            .entry(resolved_path.to_string())
            .or_default()
            .insert(format!(
                "{}:{}->{}",
                import.language, import.importer_path, import.import_spec
            ));
    }

    per_path
        .into_iter()
        .map(|(path, references)| {
            let signatures = references
                .into_iter()
                .take(MAX_CONTEXT_ITEMS)
                .collect::<Vec<_>>()
                .join("|");
            (path, signatures)
        })
        .collect()
}

fn l2_behavior_profile_for_symbol(
    symbol: &repobrain_ingest::IndexedSymbol,
    import_signature: &str,
    symbol_context_signature: &str,
    neighbor_signature: &str,
) -> L2BehaviorProfile {
    let declaration_signature = l2_declaration_signature(symbol);
    let call_signature = l2_import_role_signature(import_signature);
    let control_signature = neighbor_signature.to_string();
    let literal_signature = format!(
        "evidence={};line_bucket={}",
        symbol_evidence_label(symbol.evidence_class),
        symbol.line_number / 10
    );
    let normalized_window_hash = format!(
        "{:016x}",
        l2_fnv1a_hash(
            format!(
                "{declaration_signature}|{call_signature}|{control_signature}|{literal_signature}|{symbol_context_signature}"
            )
            .as_bytes()
        )
    );

    L2BehaviorProfile {
        declaration_signature,
        call_signature,
        control_signature,
        literal_signature,
        normalized_window_hash,
    }
}

fn l2_declaration_signature(symbol: &repobrain_ingest::IndexedSymbol) -> String {
    format!(
        "kind=function|language={}|name_shape={}",
        symbol.language,
        l2_identifier_shape(&symbol.name)
    )
}

fn l2_import_role_signature(import_signature: &str) -> String {
    const MAX_IMPORT_ROLES: usize = 16;

    let mut roles = BTreeSet::<String>::new();
    for import in import_signature.split('|') {
        let Some((_, spec_with_resolved)) = import.split_once(':') else {
            continue;
        };
        let spec = spec_with_resolved
            .split_once("->")
            .map_or(spec_with_resolved, |(value, _)| value);
        let leaf = spec
            .split([':', '/', '.'])
            .rfind(|part| !part.is_empty())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if !leaf.is_empty() {
            roles.insert(leaf);
        }
    }

    roles
        .into_iter()
        .take(MAX_IMPORT_ROLES)
        .collect::<Vec<_>>()
        .join("|")
}

fn l2_function_neighbor_signatures(
    snapshot: &RepositoryInventorySnapshot,
) -> BTreeMap<String, String> {
    let mut per_path = BTreeMap::<String, Vec<&repobrain_ingest::IndexedSymbol>>::new();
    for symbol in &snapshot.symbols {
        if matches!(symbol.kind, SymbolKind::Function) {
            per_path
                .entry(symbol.relative_path.clone())
                .or_default()
                .push(symbol);
        }
    }

    let mut neighbors = BTreeMap::<String, String>::new();
    for symbols in per_path.values_mut() {
        symbols.sort_unstable_by(|left, right| {
            left.line_number
                .cmp(&right.line_number)
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.fact_id.cmp(&right.fact_id))
        });
        let total = symbols.len();
        for (index, symbol) in symbols.iter().enumerate() {
            let previous = index
                .checked_sub(1)
                .and_then(|offset| symbols.get(offset))
                .map_or_else(
                    || "none".to_string(),
                    |value| l2_identifier_shape(&value.name),
                );
            let next = symbols.get(index + 1).map_or_else(
                || "none".to_string(),
                |value| l2_identifier_shape(&value.name),
            );
            let signature = format!("function_count={total};prev={previous};next={next}");
            neighbors.insert(symbol.fact_id.clone(), signature);
        }
    }

    neighbors
}

fn l2_identifier_shape(value: &str) -> String {
    let normalized = normalize_identifier(value);
    let tokens = normalized
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if tokens.is_empty() {
        return "tokens=0;pattern=none;digits=0".to_string();
    }

    let token_count = tokens.len();
    let pattern = tokens
        .iter()
        .take(4)
        .map(|token| l2_identifier_token_bucket(token.len()))
        .collect::<String>();
    let extra_tokens = token_count.saturating_sub(4);
    let digit_count = normalized.chars().filter(char::is_ascii_digit).count();
    format!("tokens={token_count};pattern={pattern};extra={extra_tokens};digits={digit_count}")
}

fn l2_identifier_token_bucket(token_len: usize) -> char {
    match token_len {
        0..=3 => 's',
        4..=6 => 'm',
        _ => 'l',
    }
}

pub(super) fn l2_fnv1a_hash(bytes: &[u8]) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn l2_import_context_signatures(
    snapshot: &RepositoryInventorySnapshot,
) -> BTreeMap<String, String> {
    const MAX_IMPORT_CONTEXT_ITEMS: usize = 24;

    let mut per_path = BTreeMap::<String, BTreeSet<String>>::new();
    for import in &snapshot.imports {
        let resolved_path = import.resolved_path.as_deref().unwrap_or("<unresolved>");
        let _ = per_path
            .entry(import.importer_path.clone())
            .or_default()
            .insert(format!(
                "{}:{}->{}",
                import.language, import.import_spec, resolved_path
            ));
    }

    per_path
        .into_iter()
        .map(|(path, imports)| {
            let joined = imports
                .into_iter()
                .take(MAX_IMPORT_CONTEXT_ITEMS)
                .collect::<Vec<_>>()
                .join("|");
            (path, joined)
        })
        .collect()
}

fn l2_symbol_context_signatures(
    snapshot: &RepositoryInventorySnapshot,
) -> BTreeMap<String, String> {
    let mut per_path = BTreeMap::<String, BTreeMap<String, usize>>::new();
    for symbol in &snapshot.symbols {
        let entry = per_path.entry(symbol.relative_path.clone()).or_default();
        *entry
            .entry(symbol_kind_label(symbol.kind).to_string())
            .or_insert(0) += 1;
    }

    per_path
        .into_iter()
        .map(|(path, counts)| {
            let signature = counts
                .into_iter()
                .map(|(kind, count)| format!("{kind}:{count}"))
                .collect::<Vec<_>>()
                .join("|");
            (path, signature)
        })
        .collect()
}

pub(super) fn l2_align_functions(
    reference_signals: &[L2FunctionSignal],
    target_signals: &[L2FunctionSignal],
    policy: L2RelationalPolicy,
    deadline: Instant,
) -> L2AlignmentResult {
    let mut pairs = Vec::new();
    let mut unmatched_reference = (0..reference_signals.len()).collect::<BTreeSet<_>>();
    let mut unmatched_target = (0..target_signals.len()).collect::<BTreeSet<_>>();
    let mut timed_out = false;

    if l2_pair_exact_matches(
        reference_signals,
        target_signals,
        &mut unmatched_reference,
        &mut unmatched_target,
        &mut pairs,
        policy.max_pairs,
        deadline,
    ) {
        timed_out = true;
    }
    if !timed_out
        && l2_pair_behavioral_profiles(
            reference_signals,
            target_signals,
            &mut unmatched_reference,
            &mut unmatched_target,
            &mut pairs,
            policy.max_pairs,
            deadline,
        )
    {
        timed_out = true;
    }
    if !timed_out
        && l2_pair_same_name(
            reference_signals,
            target_signals,
            &mut unmatched_reference,
            &mut unmatched_target,
            &mut pairs,
            policy.max_pairs,
            deadline,
        )
    {
        timed_out = true;
    }
    if !timed_out
        && matches!(policy.alignment_mode, CliL2AlignmentMode::SignatureAware)
        && l2_pair_signature_similarity(
            reference_signals,
            target_signals,
            &mut unmatched_reference,
            &mut unmatched_target,
            &mut pairs,
            policy,
            deadline,
        )
    {
        timed_out = true;
    }

    let unmatched_reference = unmatched_reference
        .into_iter()
        .map(|index| reference_signals[index].clone())
        .collect::<Vec<_>>();
    let unmatched_target = unmatched_target
        .into_iter()
        .map(|index| target_signals[index].clone())
        .collect::<Vec<_>>();

    L2AlignmentResult {
        pairs,
        unmatched_reference,
        unmatched_target,
        timed_out,
    }
}

fn l2_pair_exact_matches(
    reference_signals: &[L2FunctionSignal],
    target_signals: &[L2FunctionSignal],
    unmatched_reference: &mut BTreeSet<usize>,
    unmatched_target: &mut BTreeSet<usize>,
    pairs: &mut Vec<L2AlignedPair>,
    max_pairs: usize,
    deadline: Instant,
) -> bool {
    let mut deadline_probe = 0_usize;
    let mut reference_by_key = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_reference.iter().copied() {
        let signal = &reference_signals[index];
        reference_by_key
            .entry(format!(
                "{}|{}|{}",
                signal.relative_path, signal.language, signal.name
            ))
            .or_default()
            .push(index);
    }

    let mut target_by_key = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_target.iter().copied() {
        let signal = &target_signals[index];
        target_by_key
            .entry(format!(
                "{}|{}|{}",
                signal.relative_path, signal.language, signal.name
            ))
            .or_default()
            .push(index);
    }

    for (key, reference_indices) in &reference_by_key {
        let Some(target_indices) = target_by_key.get(key) else {
            continue;
        };
        let pairs_for_key = reference_indices.len().min(target_indices.len());
        for i in 0..pairs_for_key {
            if pairs.len() >= max_pairs {
                return false;
            }
            if l2_deadline_reached(deadline, &mut deadline_probe) {
                return true;
            }
            let reference_index = reference_indices[i];
            let target_index = target_indices[i];
            if unmatched_reference.remove(&reference_index)
                && unmatched_target.remove(&target_index)
            {
                pairs.push(L2AlignedPair {
                    reference: reference_signals[reference_index].clone(),
                    target: target_signals[target_index].clone(),
                    strategy: "exact_path_name",
                    score: 100,
                });
            }
        }
    }

    false
}

fn l2_pair_behavioral_profiles(
    reference_signals: &[L2FunctionSignal],
    target_signals: &[L2FunctionSignal],
    unmatched_reference: &mut BTreeSet<usize>,
    unmatched_target: &mut BTreeSet<usize>,
    pairs: &mut Vec<L2AlignedPair>,
    max_pairs: usize,
    deadline: Instant,
) -> bool {
    let mut deadline_probe = 0_usize;
    let mut reference_by_profile = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_reference.iter().copied() {
        let signal = &reference_signals[index];
        reference_by_profile
            .entry(signal.behavior_signature.clone())
            .or_default()
            .push(index);
    }

    let mut target_by_profile = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_target.iter().copied() {
        let signal = &target_signals[index];
        target_by_profile
            .entry(signal.behavior_signature.clone())
            .or_default()
            .push(index);
    }

    for (profile, reference_indices) in &reference_by_profile {
        let Some(target_indices) = target_by_profile.get(profile) else {
            continue;
        };
        let pair_count = reference_indices.len().min(target_indices.len());
        for offset in 0..pair_count {
            if pairs.len() >= max_pairs {
                return false;
            }
            if l2_deadline_reached(deadline, &mut deadline_probe) {
                return true;
            }
            let reference_index = reference_indices[offset];
            let target_index = target_indices[offset];
            if unmatched_reference.remove(&reference_index)
                && unmatched_target.remove(&target_index)
            {
                pairs.push(L2AlignedPair {
                    reference: reference_signals[reference_index].clone(),
                    target: target_signals[target_index].clone(),
                    strategy: "behavioral_profile_exact",
                    score: 96,
                });
            }
        }
    }

    false
}

fn l2_pair_same_name(
    reference_signals: &[L2FunctionSignal],
    target_signals: &[L2FunctionSignal],
    unmatched_reference: &mut BTreeSet<usize>,
    unmatched_target: &mut BTreeSet<usize>,
    pairs: &mut Vec<L2AlignedPair>,
    max_pairs: usize,
    deadline: Instant,
) -> bool {
    let mut deadline_probe = 0_usize;
    let mut reference_by_key = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_reference.iter().copied() {
        let signal = &reference_signals[index];
        reference_by_key
            .entry(format!("{}|{}", signal.language, signal.name_key))
            .or_default()
            .push(index);
    }
    let mut target_by_key = BTreeMap::<String, Vec<usize>>::new();
    for index in unmatched_target.iter().copied() {
        let signal = &target_signals[index];
        target_by_key
            .entry(format!("{}|{}", signal.language, signal.name_key))
            .or_default()
            .push(index);
    }

    for (key, reference_indices) in &reference_by_key {
        let Some(target_indices) = target_by_key.get(key) else {
            continue;
        };
        let pairs_for_key = reference_indices.len().min(target_indices.len());
        for i in 0..pairs_for_key {
            if pairs.len() >= max_pairs {
                return false;
            }
            if l2_deadline_reached(deadline, &mut deadline_probe) {
                return true;
            }
            let reference_index = reference_indices[i];
            let target_index = target_indices[i];
            if unmatched_reference.remove(&reference_index)
                && unmatched_target.remove(&target_index)
            {
                pairs.push(L2AlignedPair {
                    reference: reference_signals[reference_index].clone(),
                    target: target_signals[target_index].clone(),
                    strategy: "same_name_language",
                    score: 85,
                });
            }
        }
    }

    false
}

fn l2_pair_signature_similarity(
    reference_signals: &[L2FunctionSignal],
    target_signals: &[L2FunctionSignal],
    unmatched_reference: &mut BTreeSet<usize>,
    unmatched_target: &mut BTreeSet<usize>,
    pairs: &mut Vec<L2AlignedPair>,
    policy: L2RelationalPolicy,
    deadline: Instant,
) -> bool {
    let mut deadline_probe = 0_usize;
    let mut candidates = Vec::<(u16, usize, usize)>::new();
    for reference_index in unmatched_reference.iter().copied() {
        for target_index in unmatched_target.iter().copied() {
            if l2_deadline_reached(deadline, &mut deadline_probe) {
                return true;
            }
            let score = l2_alignment_similarity_score(
                &reference_signals[reference_index],
                &target_signals[target_index],
            );
            if score >= u16::from(policy.min_alignment_score) {
                candidates.push((score, reference_index, target_index));
            }
        }
    }

    candidates.sort_unstable_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| {
                reference_signals[left.1]
                    .id
                    .cmp(&reference_signals[right.1].id)
            })
            .then_with(|| target_signals[left.2].id.cmp(&target_signals[right.2].id))
    });

    for (score, reference_index, target_index) in candidates {
        if pairs.len() >= policy.max_pairs {
            return false;
        }
        if l2_deadline_reached(deadline, &mut deadline_probe) {
            return true;
        }
        if unmatched_reference.remove(&reference_index) && unmatched_target.remove(&target_index) {
            pairs.push(L2AlignedPair {
                reference: reference_signals[reference_index].clone(),
                target: target_signals[target_index].clone(),
                strategy: "signature_similarity",
                score,
            });
        }
    }

    false
}

fn l2_alignment_similarity_score(reference: &L2FunctionSignal, target: &L2FunctionSignal) -> u16 {
    if reference.language != target.language {
        return 0;
    }

    let mut score = 0_u16;
    if reference.behavior_profile.declaration_signature
        == target.behavior_profile.declaration_signature
    {
        score += 24;
    }
    if !reference.behavior_profile.call_signature.is_empty()
        && reference.behavior_profile.call_signature == target.behavior_profile.call_signature
    {
        score += 20;
    }
    if reference.behavior_profile.control_signature == target.behavior_profile.control_signature {
        score += 10;
    }
    if reference.behavior_profile.literal_signature == target.behavior_profile.literal_signature {
        score += 8;
    }
    if reference.behavior_profile.normalized_window_hash
        == target.behavior_profile.normalized_window_hash
    {
        score += 16;
    }
    if !reference.import_signature.is_empty()
        && reference.import_signature == target.import_signature
    {
        score += 10;
    }
    if !reference.reverse_import_signature.is_empty()
        && reference.reverse_import_signature == target.reverse_import_signature
    {
        score += 4;
    }
    if !reference.symbol_context_signature.is_empty()
        && reference.symbol_context_signature == target.symbol_context_signature
    {
        score += 6;
    }
    if l2_file_stem(&reference.relative_path) == l2_file_stem(&target.relative_path) {
        score += 4;
    }
    if l2_parent_path(&reference.relative_path) == l2_parent_path(&target.relative_path) {
        score += 2;
    }
    score += identifier_similarity_score(&reference.name_key, &target.name_key);

    let line_distance = reference.line_number.abs_diff(target.line_number);
    if line_distance <= 3 {
        score += 3;
    } else if line_distance <= 12 {
        score += 2;
    }

    score.min(100)
}

fn l2_file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

fn l2_parent_path(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default()
}

fn normalize_identifier(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = true;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !previous_was_separator {
                normalized.push(' ');
            }
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push(' ');
            previous_was_separator = true;
        }
    }

    normalized.trim().to_string()
}

fn identifier_similarity_score(left: &str, right: &str) -> u16 {
    let left_tokens = left
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();
    let right_tokens = right
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>();

    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0;
    }

    let intersection = left_tokens.intersection(&right_tokens).count();
    let union = left_tokens.union(&right_tokens).count();
    let scaled = (intersection * 20 + (union / 2)) / union;

    u16::try_from(scaled).unwrap_or(20)
}

fn l2_relational_outcome(
    alignment: &L2AlignmentResult,
    delta: &SemanticDelta,
) -> L2RelationalOutcome {
    let mut differing_pairs = 0_usize;
    let mut unchanged_pairs = 0_usize;
    let mut witness = None;

    for pair in &alignment.pairs {
        let mut difference_reasons = l2_pair_difference_reasons(pair);
        if difference_reasons.is_empty() && l2_pair_requires_conservative_difference(pair, delta) {
            difference_reasons.push("modified_file_without_semantic_signal");
        }
        if difference_reasons.is_empty() {
            unchanged_pairs += 1;
            continue;
        }

        differing_pairs += 1;
        if witness.is_none() {
            witness = Some(format!(
                "l2_pair_diff:{}=>{} strategy={} score={} reasons={}",
                pair.reference.id,
                pair.target.id,
                pair.strategy,
                pair.score,
                difference_reasons.join(",")
            ));
        }
    }

    if witness.is_none() {
        if let Some(reference_only) = alignment.unmatched_reference.first() {
            witness = Some(format!("l2_unmatched_reference:{}", reference_only.id));
        } else if let Some(target_only) = alignment.unmatched_target.first() {
            witness = Some(format!("l2_unmatched_target:{}", target_only.id));
        } else if alignment.timed_out {
            witness = Some("l2_timeout".to_string());
        } else {
            witness = semantic_delta_witness(delta).map(|value| format!("l2_scope:{value}"));
        }
    }

    L2RelationalOutcome {
        compared_pairs: alignment.pairs.len(),
        differing_pairs,
        unchanged_pairs,
        unmatched_reference: alignment.unmatched_reference.len(),
        unmatched_target: alignment.unmatched_target.len(),
        timed_out: alignment.timed_out,
        witness,
    }
}

fn l2_pair_requires_conservative_difference(pair: &L2AlignedPair, delta: &SemanticDelta) -> bool {
    pair.reference.relative_path == pair.target.relative_path
        && pair.reference.name_key == pair.target.name_key
        && delta.modified_files.contains(&pair.reference.relative_path)
}

fn l2_pair_difference_reasons(pair: &L2AlignedPair) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if pair.reference.behavior_profile.declaration_signature
        != pair.target.behavior_profile.declaration_signature
    {
        reasons.push("declaration_signature");
    }
    if pair.reference.behavior_profile.call_signature != pair.target.behavior_profile.call_signature
    {
        reasons.push("call_signature");
    }
    if pair.reference.behavior_profile.control_signature
        != pair.target.behavior_profile.control_signature
    {
        reasons.push("control_signature");
    }
    if pair.reference.behavior_profile.literal_signature
        != pair.target.behavior_profile.literal_signature
    {
        reasons.push("literal_signature");
    }
    if pair.reference.behavior_profile.normalized_window_hash
        != pair.target.behavior_profile.normalized_window_hash
    {
        reasons.push("window_hash");
    }
    if pair.reference.import_signature != pair.target.import_signature {
        reasons.push("import_signature");
    }
    if pair.reference.symbol_context_signature != pair.target.symbol_context_signature {
        reasons.push("symbol_context_signature");
    }

    reasons
}

fn l2_status_from_outcome(outcome: &L2RelationalOutcome) -> EquivalenceEvidenceStatus {
    if outcome.timed_out {
        return EquivalenceEvidenceStatus::Inconclusive;
    }
    if outcome.differing_pairs > 0
        || outcome.unmatched_reference > 0
        || outcome.unmatched_target > 0
    {
        return EquivalenceEvidenceStatus::ObservedDifference;
    }
    if outcome.compared_pairs > 0 {
        return EquivalenceEvidenceStatus::NoDifferenceObserved;
    }

    EquivalenceEvidenceStatus::Inconclusive
}

fn l2_receipt(
    status: EquivalenceEvidenceStatus,
    bounds: String,
    timeout_ms: u64,
    assumptions: Vec<String>,
    witness: Option<String>,
) -> EquivalenceEvidenceReceipt {
    EquivalenceEvidenceReceipt {
        backend: "l2_relational_semantic".to_string(),
        stage: EquivalenceStage::L2RelationalSemantic,
        status,
        solver: Some("relational_profile_checker".to_string()),
        bounds: Some(bounds),
        timeout_ms: Some(timeout_ms),
        assumptions,
        witness,
    }
}

pub(crate) fn l2_status_label(status: EquivalenceEvidenceStatus) -> &'static str {
    match status {
        EquivalenceEvidenceStatus::ObservedDifference => "l2_relational_counterexample",
        EquivalenceEvidenceStatus::NoDifferenceObserved => "l2_relational_no_counterexample",
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => "reference_snapshot_missing",
        EquivalenceEvidenceStatus::Inconclusive => "l2_relational_inconclusive",
    }
}

pub(super) fn l2_summary(
    receipt: &EquivalenceEvidenceReceipt,
    delta: &SemanticDelta,
    reference_snapshot_id: &str,
    target_snapshot_id: &str,
) -> String {
    let structural = semantic_diff_summary(delta, reference_snapshot_id, target_snapshot_id);
    match receipt.status {
        EquivalenceEvidenceStatus::NoDifferenceObserved => format!(
            "L2 relational semantic backend observed no difference across aligned function pairs under configured bounds. Structural baseline: {structural}"
        ),
        EquivalenceEvidenceStatus::ObservedDifference => format!(
            "L2 relational semantic backend observed bounded relational differences (paired or unmatched function evidence). Structural baseline: {structural}"
        ),
        EquivalenceEvidenceStatus::ReferenceSnapshotMissing => {
            "reference snapshot is missing; relational semantic backend was not executed"
                .to_string()
        }
        EquivalenceEvidenceStatus::Inconclusive => format!(
            "L2 relational semantic backend was inconclusive under configured bounds or timeout; no global semantic proof is implied. Structural baseline: {structural}"
        ),
    }
}

pub(super) fn semantic_delta_witness(delta: &SemanticDelta) -> Option<String> {
    if !delta.has_changes() {
        return None;
    }

    if let Some(path) = delta.changed_scope.first() {
        return Some(format!("changed_scope:{path}"));
    }

    if let Some(target) = delta.added_verification_targets.first() {
        return Some(format!("added_verification_target:{target}"));
    }

    delta
        .removed_verification_targets
        .first()
        .map(|target| format!("removed_verification_target:{target}"))
}

const L2_DEADLINE_CHECK_INTERVAL: usize = 64;

fn l2_deadline_reached(deadline: Instant, probe_counter: &mut usize) -> bool {
    *probe_counter = probe_counter.saturating_add(1);
    if *probe_counter == 1 || (*probe_counter).is_multiple_of(L2_DEADLINE_CHECK_INTERVAL) {
        return Instant::now() >= deadline;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(
        id: &str,
        name: &str,
        name_key: &str,
        path: &str,
        behavior_signature: &str,
        line_number: u32,
    ) -> L2FunctionSignal {
        L2FunctionSignal {
            id: id.to_string(),
            name: name.to_string(),
            name_key: name_key.to_string(),
            relative_path: path.to_string(),
            manifest_path: Some("Cargo.toml".to_string()),
            language: "rust".to_string(),
            line_number,
            import_signature: "std::fmt".to_string(),
            reverse_import_signature: "crate::entry".to_string(),
            symbol_context_signature: "function:1".to_string(),
            behavior_profile: L2BehaviorProfile {
                declaration_signature: "fn(i32)->i32".to_string(),
                call_signature: "map|collect".to_string(),
                control_signature: "if|match".to_string(),
                literal_signature: "1|2|3".to_string(),
                normalized_window_hash: "abcd1234".to_string(),
            },
            behavior_signature: behavior_signature.to_string(),
        }
    }

    #[test]
    fn exact_only_alignment_leaves_non_exact_candidates_unmatched() {
        let reference = vec![signal(
            "reference:normalize@src/a.rs:10",
            "normalize_order",
            "normalize order",
            "src/a.rs",
            "behavior_a",
            10,
        )];
        let target = vec![signal(
            "target:prepare@src/b.rs:12",
            "prepare_invoice",
            "prepare invoice",
            "src/b.rs",
            "behavior_b",
            12,
        )];
        let policy = L2RelationalPolicy {
            max_pairs: 8,
            max_candidate_functions: 16,
            timeout_ms: 5_000,
            alignment_mode: CliL2AlignmentMode::ExactOnly,
            min_alignment_score: 65,
        };
        let deadline = Instant::now() + Duration::from_millis(500);

        let result = l2_align_functions(&reference, &target, policy, deadline);

        assert!(result.pairs.is_empty());
        assert_eq!(result.unmatched_reference.len(), 1);
        assert_eq!(result.unmatched_target.len(), 1);
        assert!(!result.timed_out);
    }

    #[test]
    fn signature_aware_alignment_can_match_renamed_behaviorally_similar_functions() {
        let reference = vec![signal(
            "reference:process_order@src/order.rs:30",
            "process_order",
            "process order",
            "src/order.rs",
            "behavior_reference",
            30,
        )];
        let target = vec![signal(
            "target:process_order_v2@src/order_new.rs:31",
            "reconcile_billing",
            "reconcile billing",
            "src/order_new.rs",
            "behavior_target",
            31,
        )];
        let policy = L2RelationalPolicy {
            max_pairs: 8,
            max_candidate_functions: 16,
            timeout_ms: 5_000,
            alignment_mode: CliL2AlignmentMode::SignatureAware,
            min_alignment_score: 65,
        };
        let deadline = Instant::now() + Duration::from_millis(500);

        let result = l2_align_functions(&reference, &target, policy, deadline);

        assert_eq!(result.pairs.len(), 1);
        assert_eq!(result.pairs[0].strategy, "signature_similarity");
        assert!(result.pairs[0].score >= 65);
        assert!(result.unmatched_reference.is_empty());
        assert!(result.unmatched_target.is_empty());
        assert!(!result.timed_out);
    }

    #[test]
    fn identifier_shape_retains_entropy_for_different_names() {
        let left = l2_identifier_shape("process_order");
        let right = l2_identifier_shape("validate_payment");
        let single = l2_identifier_shape("handler");

        assert_ne!(left, right);
        assert!(left.contains("pattern="));
        assert!(right.contains("pattern="));
        assert!(single.contains("tokens=1"));
    }
}
