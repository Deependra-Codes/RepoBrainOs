use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Class,
    Struct,
    Enum,
    Trait,
    Interface,
    TypeAlias,
    Constant,
    Module,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolEvidenceClass {
    LexicalConfirmed,
    SyntaxConfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedSymbol {
    #[serde(default)]
    pub fact_id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub relative_path: String,
    pub language: String,
    pub line_number: u32,
    pub evidence_class: SymbolEvidenceClass,
}

impl IndexedSymbol {
    pub(crate) fn new(
        name: impl Into<String>,
        kind: SymbolKind,
        relative_path: impl Into<String>,
        language: impl Into<String>,
        line_number: u32,
        evidence_class: SymbolEvidenceClass,
    ) -> Self {
        let name = name.into();
        let relative_path = relative_path.into();
        let language = language.into();

        Self {
            fact_id: symbol_fact_id(&name, kind, &relative_path, &language, line_number),
            name,
            kind,
            relative_path,
            language,
            line_number,
            evidence_class,
        }
    }

    pub(crate) fn backfill_fact_id(&mut self) {
        if self.fact_id.is_empty() {
            self.fact_id = symbol_fact_id(
                &self.name,
                self.kind,
                &self.relative_path,
                &self.language,
                self.line_number,
            );
        }
    }
}

pub(crate) fn supports_symbol_extraction(language: &str) -> bool {
    matches!(language, "javascript" | "python" | "rust" | "typescript")
}

pub(crate) fn extract_symbols_for_file(
    relative_path: &str,
    language: &str,
    contents: &str,
) -> Vec<IndexedSymbol> {
    match language {
        "rust" => extract_rust_symbols_for_file(relative_path, contents),
        "python" | "typescript" | "javascript" => {
            extract_line_symbols_for_file(relative_path, language, contents)
        }
        _ => Vec::new(),
    }
}

fn extract_rust_symbols_for_file(relative_path: &str, contents: &str) -> Vec<IndexedSymbol> {
    let mut symbols = Vec::new();
    let mut cfg_test_pending = false;
    let mut brace_depth = 0_usize;
    let mut skip_test_module_depth = None;

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        let next_brace_depth = update_brace_depth(brace_depth, trimmed);

        if let Some(skip_depth) = skip_test_module_depth {
            brace_depth = next_brace_depth;
            if brace_depth == skip_depth {
                skip_test_module_depth = None;
            }
            continue;
        }

        if trimmed.starts_with("#[cfg(test)]") {
            cfg_test_pending = true;
            brace_depth = next_brace_depth;
            continue;
        }

        if cfg_test_pending && trimmed.starts_with("mod tests") && trimmed.contains('{') {
            skip_test_module_depth = Some(brace_depth);
            cfg_test_pending = false;
            brace_depth = next_brace_depth;
            continue;
        }

        if cfg_test_pending && !trimmed.is_empty() && !trimmed.starts_with("#[") {
            cfg_test_pending = false;
        }

        if trimmed.is_empty() || is_comment_line("rust", trimmed) {
            brace_depth = next_brace_depth;
            continue;
        }

        if let Some((kind, name)) = extract_rust_symbol(trimmed) {
            symbols.push(IndexedSymbol::new(
                name,
                kind,
                relative_path,
                "rust",
                line_number(line_index),
                SymbolEvidenceClass::LexicalConfirmed,
            ));
        }

        brace_depth = next_brace_depth;
    }

    symbols
}

fn extract_line_symbols_for_file(
    relative_path: &str,
    language: &str,
    contents: &str,
) -> Vec<IndexedSymbol> {
    let mut symbols = Vec::new();

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || is_comment_line(language, trimmed) {
            continue;
        }

        let extracted = match language {
            "rust" => extract_rust_symbol(trimmed),
            "python" => extract_python_symbol(trimmed),
            "typescript" | "javascript" => extract_typescript_symbol(trimmed),
            _ => None,
        };

        if let Some((kind, name)) = extracted {
            symbols.push(IndexedSymbol::new(
                name,
                kind,
                relative_path,
                language,
                line_number(line_index),
                SymbolEvidenceClass::LexicalConfirmed,
            ));
        }
    }

    symbols
}

pub(crate) fn symbol_fact_id(
    name: &str,
    kind: SymbolKind,
    relative_path: &str,
    language: &str,
    line_number: u32,
) -> String {
    format!(
        "sym_{}_{}_{}_{}_{}",
        sanitize_for_id(language),
        sanitize_for_id(relative_path),
        line_number,
        symbol_kind_label(kind),
        sanitize_for_id(name)
    )
}

fn extract_rust_symbol(trimmed: &str) -> Option<(SymbolKind, String)> {
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
    let tokens = skip_rust_prefixes(&tokens);

    match tokens {
        ["const", "fn", name, ..] | ["fn", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Function, name))
        }
        ["struct", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Struct, name)),
        ["enum", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Enum, name)),
        ["trait", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Trait, name)),
        ["type", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::TypeAlias, name)),
        ["const" | "static", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Constant, name))
        }
        ["mod", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Module, name)),
        _ => None,
    }
}

fn extract_python_symbol(trimmed: &str) -> Option<(SymbolKind, String)> {
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();

    match tokens.as_slice() {
        ["async", "def", name, ..] | ["def", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Function, name))
        }
        ["class", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Class, name)),
        _ => None,
    }
}

fn extract_typescript_symbol(trimmed: &str) -> Option<(SymbolKind, String)> {
    let tokens = trimmed.split_whitespace().collect::<Vec<_>>();
    let tokens = skip_typescript_prefixes(&tokens);

    match tokens {
        ["async", "function", name, ..] | ["function", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Function, name))
        }
        ["class", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::Class, name)),
        ["interface", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Interface, name))
        }
        ["type", name, ..] => identifier_from_token(name).map(|name| (SymbolKind::TypeAlias, name)),
        ["const", "enum", name, ..] | ["enum", name, ..] => {
            identifier_from_token(name).map(|name| (SymbolKind::Enum, name))
        }
        _ => None,
    }
}

fn skip_rust_prefixes<'a>(tokens: &'a [&str]) -> &'a [&'a str] {
    let mut start = 0;

    while let Some(token) = tokens.get(start) {
        if *token == "pub"
            || token.starts_with("pub(")
            || matches!(*token, "async" | "default" | "unsafe")
        {
            start += 1;
        } else {
            break;
        }
    }

    &tokens[start..]
}

fn skip_typescript_prefixes<'a>(tokens: &'a [&str]) -> &'a [&'a str] {
    let mut start = 0;

    while let Some(token) = tokens.get(start) {
        if matches!(*token, "abstract" | "declare" | "default" | "export") {
            start += 1;
        } else {
            break;
        }
    }

    &tokens[start..]
}

fn identifier_from_token(token: &str) -> Option<String> {
    let token = token.strip_prefix("r#").unwrap_or(token);
    let identifier = token
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        .collect::<String>();

    if identifier.is_empty() {
        None
    } else {
        Some(identifier)
    }
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

fn update_brace_depth(current_depth: usize, line: &str) -> usize {
    let opening = line.chars().filter(|character| *character == '{').count();
    let closing = line.chars().filter(|character| *character == '}').count();

    current_depth + opening - closing.min(current_depth + opening)
}

fn symbol_kind_label(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Function => "function",
        SymbolKind::Class => "class",
        SymbolKind::Struct => "struct",
        SymbolKind::Enum => "enum",
        SymbolKind::Trait => "trait",
        SymbolKind::Interface => "interface",
        SymbolKind::TypeAlias => "type_alias",
        SymbolKind::Constant => "constant",
        SymbolKind::Module => "module",
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
    use super::{SymbolEvidenceClass, SymbolKind, extract_symbols_for_file, symbol_fact_id};

    #[test]
    fn rust_extraction_stays_lexical_and_collects_main_declarations() {
        let symbols = extract_symbols_for_file(
            "src/lib.rs",
            "rust",
            r#"
pub struct RepositoryScanner {
}

pub enum RefreshMode {
}

pub const VERSION: &str = "1";

pub async fn scan() {}
"#,
        );

        assert_eq!(symbols.len(), 4);
        assert_eq!(symbols[0].name, "RepositoryScanner");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
        assert_eq!(
            symbols[0].fact_id,
            symbol_fact_id(
                "RepositoryScanner",
                SymbolKind::Struct,
                "src/lib.rs",
                "rust",
                2
            )
        );
        assert_eq!(
            symbols[0].evidence_class,
            SymbolEvidenceClass::LexicalConfirmed
        );
        assert_eq!(symbols[3].name, "scan");
        assert_eq!(symbols[3].kind, SymbolKind::Function);
    }

    #[test]
    fn python_and_typescript_extraction_cover_common_shapes() {
        let python_symbols = extract_symbols_for_file(
            "src/module.py",
            "python",
            r"
class RepositoryScanner:
    pass

async def scan() -> None:
    return None
",
        );
        let ts_symbols = extract_symbols_for_file(
            "src/module.ts",
            "typescript",
            r#"
export interface SnapshotBinding {
}

export type ReadinessState = "ready";

export function scan(): void {}
"#,
        );

        assert_eq!(python_symbols.len(), 2);
        assert_eq!(python_symbols[0].kind, SymbolKind::Class);
        assert_eq!(python_symbols[1].name, "scan");
        assert_eq!(ts_symbols.len(), 3);
        assert_eq!(ts_symbols[0].kind, SymbolKind::Interface);
        assert_eq!(ts_symbols[1].kind, SymbolKind::TypeAlias);
        assert_eq!(ts_symbols[2].kind, SymbolKind::Function);
    }

    #[test]
    fn rust_extraction_skips_cfg_test_modules() {
        let symbols = extract_symbols_for_file(
            "src/lib.rs",
            "rust",
            r##"
pub struct RepositoryScanner {}

#[cfg(test)]
mod tests {
    const FIXTURE: &str = r#"
pub struct RepositoryScanner {}
"#;
}
"##,
        );

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "RepositoryScanner");
        assert_eq!(symbols[0].kind, SymbolKind::Struct);
    }
}
