use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportKind {
    Module,
    Use,
    PythonFrom,
    JSImport,
    Require,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportEvidenceClass {
    LexicalConfirmed,
    SyntaxConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedImport {
    #[serde(default)]
    pub fact_id: String,
    pub importer_path: String,
    pub import_spec: String,
    pub line_number: u32,
    pub language: String,
    pub kind: ImportKind,
    pub resolved_path: Option<String>,
    pub evidence_class: ImportEvidenceClass,
}

impl IndexedImport {
    pub(crate) fn new(
        importer_path: impl Into<String>,
        import_spec: impl Into<String>,
        line_number: u32,
        language: impl Into<String>,
        kind: ImportKind,
        resolved_path: Option<String>,
        evidence_class: ImportEvidenceClass,
    ) -> Self {
        let importer_path = importer_path.into();
        let import_spec = import_spec.into();
        let language = language.into();

        Self {
            fact_id: import_fact_id(&importer_path, &import_spec, line_number, &language, kind),
            importer_path,
            import_spec,
            line_number,
            language,
            kind,
            resolved_path,
            evidence_class,
        }
    }

    pub(crate) fn backfill_fact_id(&mut self) {
        if self.fact_id.is_empty() {
            self.fact_id = import_fact_id(
                &self.importer_path,
                &self.import_spec,
                self.line_number,
                &self.language,
                self.kind,
            );
        }
    }
}

pub(crate) fn supports_import_extraction(language: &str) -> bool {
    matches!(language, "javascript" | "python" | "rust" | "typescript")
}

pub(crate) fn extract_imports_for_file(
    relative_path: &str,
    language: &str,
    contents: &str,
    available_files: &BTreeSet<String>,
) -> Vec<IndexedImport> {
    let mut imports = Vec::new();

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || is_comment_line(language, trimmed) {
            continue;
        }

        let extracted = match language {
            "rust" => extract_rust_import(relative_path, trimmed, available_files),
            "python" => extract_python_import(relative_path, trimmed, available_files),
            "typescript" | "javascript" => {
                extract_typescript_import(relative_path, trimmed, available_files)
            }
            _ => None,
        };

        if let Some((kind, import_spec, resolved_path)) = extracted {
            imports.push(IndexedImport::new(
                relative_path,
                import_spec,
                line_number(line_index),
                language,
                kind,
                resolved_path,
                ImportEvidenceClass::LexicalConfirmed,
            ));
        }
    }

    imports
}

pub(crate) fn import_fact_id(
    importer_path: &str,
    import_spec: &str,
    line_number: u32,
    language: &str,
    kind: ImportKind,
) -> String {
    format!(
        "imp_{}_{}_{}_{}_{}",
        sanitize_for_id(language),
        sanitize_for_id(importer_path),
        line_number,
        import_kind_label(kind),
        sanitize_for_id(import_spec)
    )
}

pub(crate) fn extract_rust_import(
    relative_path: &str,
    trimmed: &str,
    available_files: &BTreeSet<String>,
) -> Option<(ImportKind, String, Option<String>)> {
    let without_prefixes = trim_known_prefixes(trimmed, &["pub ", "unsafe ", "async "]);
    if let Some(module_name) = without_prefixes
        .strip_prefix("mod ")
        .and_then(statement_head)
    {
        let resolved_path = resolve_rust_mod(relative_path, module_name, available_files);
        return Some((ImportKind::Module, module_name.to_string(), resolved_path));
    }

    let use_path = without_prefixes
        .strip_prefix("use ")
        .and_then(statement_head)?;
    let resolved_path = resolve_rust_use(relative_path, use_path, available_files);

    Some((ImportKind::Use, use_path.to_string(), resolved_path))
}

pub(crate) fn extract_python_import(
    relative_path: &str,
    trimmed: &str,
    available_files: &BTreeSet<String>,
) -> Option<(ImportKind, String, Option<String>)> {
    if let Some(module_spec) = trimmed.strip_prefix("from ").and_then(|value| {
        value
            .split_once(" import ")
            .map(|(module, _)| module.trim())
    }) {
        let resolved_path = resolve_python_from(relative_path, module_spec, available_files);
        return Some((
            ImportKind::PythonFrom,
            module_spec.to_string(),
            resolved_path,
        ));
    }

    None
}

pub(crate) fn extract_typescript_import(
    relative_path: &str,
    trimmed: &str,
    available_files: &BTreeSet<String>,
) -> Option<(ImportKind, String, Option<String>)> {
    if let Some(import_spec) = quoted_spec_after(trimmed, " from ") {
        let resolved_path = resolve_relative_module(relative_path, import_spec, available_files);
        return Some((ImportKind::JSImport, import_spec.to_string(), resolved_path));
    }

    if let Some(import_spec) = trimmed
        .strip_prefix("import ")
        .and_then(quoted_spec_at_start)
    {
        let resolved_path = resolve_relative_module(relative_path, import_spec, available_files);
        return Some((ImportKind::JSImport, import_spec.to_string(), resolved_path));
    }

    if let Some(import_spec) = quoted_spec_after(trimmed, "require(") {
        let resolved_path = resolve_relative_module(relative_path, import_spec, available_files);
        return Some((ImportKind::Require, import_spec.to_string(), resolved_path));
    }

    None
}

fn resolve_rust_mod(
    relative_path: &str,
    module_name: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    let base_dir = parent_dir(relative_path);
    let candidates = [
        join_relative_path(&base_dir, &format!("{module_name}.rs")),
        join_relative_path(&base_dir, &format!("{module_name}/mod.rs")),
    ];

    candidates
        .into_iter()
        .find(|candidate| available_files.contains(candidate))
}

fn resolve_rust_use(
    relative_path: &str,
    use_path: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    let normalized = use_path
        .split("::")
        .take_while(|segment| {
            !segment.is_empty()
                && *segment != "self"
                && *segment != "Self"
                && segment
                    .chars()
                    .next()
                    .is_some_and(|character| character.is_ascii_lowercase())
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return None;
    }

    if use_path.starts_with("crate::") {
        return resolve_rust_crate_path(relative_path, &normalized[1..], available_files);
    }

    if use_path.starts_with("super::") {
        return resolve_rust_super_path(relative_path, &normalized, available_files);
    }

    None
}

fn resolve_rust_crate_path(
    relative_path: &str,
    segments: &[&str],
    available_files: &BTreeSet<String>,
) -> Option<String> {
    let crate_src_prefix = crate_src_prefix(relative_path)?;
    resolve_rust_module_candidates(&crate_src_prefix, segments, available_files)
}

fn resolve_rust_super_path(
    relative_path: &str,
    segments: &[&str],
    available_files: &BTreeSet<String>,
) -> Option<String> {
    let relative_from_src = relative_path.split_once("/src/").map(|(_, rest)| rest)?;
    let mut base = PathBuf::from(relative_from_src);
    let _ = base.pop();
    let mut remaining = segments;

    while remaining.first().is_some_and(|segment| *segment == "super") {
        let _ = base.pop();
        remaining = &remaining[1..];
    }

    let crate_src_prefix = crate_src_prefix(relative_path)?;
    let base = join_relative_path(
        &crate_src_prefix,
        &normalize_relative_path(&base.to_string_lossy()),
    );
    let base = if base == crate_src_prefix {
        crate_src_prefix
    } else {
        base
    };

    resolve_rust_module_candidates(&base, remaining, available_files)
}

fn resolve_rust_module_candidates(
    base_prefix: &str,
    segments: &[&str],
    available_files: &BTreeSet<String>,
) -> Option<String> {
    if segments.is_empty() {
        return None;
    }

    for prefix_len in (1..=segments.len()).rev() {
        let module_path = segments[..prefix_len].join("/");
        let candidates = [
            join_relative_path(base_prefix, &format!("{module_path}.rs")),
            join_relative_path(base_prefix, &format!("{module_path}/mod.rs")),
        ];

        if let Some(candidate) = candidates
            .into_iter()
            .find(|candidate| available_files.contains(candidate))
        {
            return Some(candidate);
        }
    }

    None
}

fn resolve_python_from(
    relative_path: &str,
    module_spec: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    if !module_spec.starts_with('.') {
        return resolve_python_module_from_root(module_spec, available_files);
    }

    let dot_count = module_spec
        .chars()
        .take_while(|character| *character == '.')
        .count();
    let remainder = &module_spec[dot_count..];
    let mut base_dir = PathBuf::from(parent_dir(relative_path));

    for _ in 1..dot_count {
        let _ = base_dir.pop();
    }

    let base = normalize_relative_path(&base_dir.to_string_lossy());
    resolve_python_module(&base, remainder, available_files)
}

fn resolve_python_module_from_root(
    module_spec: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    resolve_python_module("", module_spec, available_files)
}

fn resolve_python_module(
    base: &str,
    module_spec: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    let module_path = module_spec.replace('.', "/");
    let base_prefix = if base.is_empty() {
        module_path
    } else if module_path.is_empty() {
        base.to_string()
    } else {
        join_relative_path(base, &module_path)
    };
    let candidates = [
        format!("{base_prefix}.py"),
        join_relative_path(&base_prefix, "__init__.py"),
    ];

    candidates
        .into_iter()
        .find(|candidate| available_files.contains(candidate))
}

fn resolve_relative_module(
    relative_path: &str,
    import_spec: &str,
    available_files: &BTreeSet<String>,
) -> Option<String> {
    if !is_relative_module_spec(import_spec) {
        return None;
    }

    let parent = parent_dir(relative_path);
    let joined = normalize_path_segments(&join_relative_path(&parent, import_spec));
    let extension = Path::new(relative_path)
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();

    let mut candidates = Vec::new();
    if let Some(explicit_extension) = Path::new(&joined)
        .extension()
        .and_then(|value| value.to_str())
    {
        candidates.push(joined.clone());

        let stem = Path::new(&joined)
            .with_extension("")
            .to_string_lossy()
            .into_owned();
        let alternate_extensions = match explicit_extension {
            "js" | "jsx" | "ts" | "tsx" => ["ts", "tsx", "js", "jsx"].as_slice(),
            _ => &[][..],
        };

        for extension in alternate_extensions {
            candidates.push(format!("{stem}.{extension}"));
        }
    }

    let likely_extensions = match extension {
        "js" | "jsx" => ["js", "jsx", "ts", "tsx"].as_slice(),
        _ => ["ts", "tsx", "js", "jsx"].as_slice(),
    };

    for extension in likely_extensions {
        candidates.push(format!("{joined}.{extension}"));
        candidates.push(join_relative_path(&joined, &format!("index.{extension}")));
    }

    candidates
        .into_iter()
        .find(|candidate| available_files.contains(candidate))
}

fn quoted_spec_after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    let start = line.find(marker)? + marker.len();
    quoted_spec_at_start(&line[start..])
}

fn quoted_spec_at_start(value: &str) -> Option<&str> {
    let quote = value.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let rest = &value[quote.len_utf8()..];
    let end = rest.find(quote)?;

    Some(&rest[..end])
}

fn statement_head(value: &str) -> Option<&str> {
    value.split(';').next().map(str::trim)
}

fn trim_known_prefixes<'a>(value: &'a str, prefixes: &[&str]) -> &'a str {
    let mut trimmed = value;

    loop {
        let mut matched = false;
        for prefix in prefixes {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                trimmed = rest;
                matched = true;
                break;
            }
        }
        if !matched {
            return trimmed;
        }
    }
}

fn parent_dir(relative_path: &str) -> String {
    let path = Path::new(relative_path);
    match path.parent() {
        Some(parent) => normalize_relative_path(&parent.to_string_lossy()),
        None => String::new(),
    }
}

fn crate_src_prefix(relative_path: &str) -> Option<String> {
    if relative_path.starts_with("src/") {
        return Some("src".to_string());
    }

    let (prefix, _) = relative_path.split_once("/src/")?;
    Some(format!("{prefix}/src"))
}

fn join_relative_path(base: &str, child: &str) -> String {
    if base.is_empty() {
        normalize_path_segments(child)
    } else if child.is_empty() {
        normalize_path_segments(base)
    } else {
        normalize_path_segments(&format!("{base}/{child}"))
    }
}

fn normalize_path_segments(path: &str) -> String {
    let mut normalized = Vec::new();

    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                let _ = normalized.pop();
            }
            value => normalized.push(value),
        }
    }

    normalized.join("/")
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/")
}

fn is_relative_module_spec(import_spec: &str) -> bool {
    import_spec.starts_with("./") || import_spec.starts_with("../")
}

fn is_comment_line(language: &str, trimmed: &str) -> bool {
    match language {
        "python" => trimmed.starts_with('#'),
        "rust" | "typescript" | "javascript" => {
            trimmed.starts_with("//")
                || trimmed.starts_with("/*")
                || trimmed.starts_with('*')
                || trimmed.starts_with("*/")
        }
        _ => false,
    }
}

fn line_number(line_index: usize) -> u32 {
    u32::try_from(line_index + 1).unwrap_or(u32::MAX)
}

fn import_kind_label(kind: ImportKind) -> &'static str {
    match kind {
        ImportKind::Module => "module",
        ImportKind::Use => "use",
        ImportKind::PythonFrom => "python_from",
        ImportKind::JSImport => "js_import",
        ImportKind::Require => "require",
    }
}

fn sanitize_for_id(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{ImportKind, extract_imports_for_file, import_fact_id};

    #[test]
    fn rust_and_typescript_imports_resolve_local_paths() {
        let available_files = BTreeSet::from([
            "src/lib.rs".to_string(),
            "src/symbols.rs".to_string(),
            "src/tool-definitions.ts".to_string(),
            "src/contracts.ts".to_string(),
        ]);
        let rust_imports = extract_imports_for_file(
            "src/lib.rs",
            "rust",
            "mod symbols;\nuse crate::symbols::SymbolKind;\n",
            &available_files,
        );
        let ts_imports = extract_imports_for_file(
            "src/tool-definitions.ts",
            "typescript",
            "import type { ToolDefinition } from \"./contracts.js\";\n",
            &available_files,
        );

        assert_eq!(rust_imports.len(), 2);
        assert_eq!(rust_imports[0].kind, ImportKind::Module);
        assert_eq!(
            rust_imports[0].resolved_path.as_deref(),
            Some("src/symbols.rs")
        );
        assert_eq!(rust_imports[1].kind, ImportKind::Use);
        assert_eq!(
            rust_imports[1].resolved_path.as_deref(),
            Some("src/symbols.rs")
        );
        assert_eq!(
            rust_imports[1].fact_id,
            import_fact_id(
                "src/lib.rs",
                "crate::symbols::SymbolKind",
                2,
                "rust",
                ImportKind::Use
            )
        );
        assert_eq!(ts_imports.len(), 1);
        assert_eq!(
            ts_imports[0].resolved_path.as_deref(),
            Some("src/contracts.ts")
        );
    }

    #[test]
    fn python_relative_imports_resolve_locally() {
        let available_files = BTreeSet::from([
            "pkg/__init__.py".to_string(),
            "pkg/module.py".to_string(),
            "pkg/sub/consumer.py".to_string(),
        ]);
        let imports = extract_imports_for_file(
            "pkg/sub/consumer.py",
            "python",
            "from ..module import value\n",
            &available_files,
        );

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].kind, ImportKind::PythonFrom);
        assert_eq!(imports[0].resolved_path.as_deref(), Some("pkg/module.py"));
    }
}
