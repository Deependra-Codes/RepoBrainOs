use std::collections::BTreeSet;

use tree_sitter::{Node, Parser};

use crate::imports::{extract_python_import, extract_rust_import, extract_typescript_import};
use crate::symbols::{IndexedSymbol, SymbolEvidenceClass, SymbolKind};
use crate::{ImportEvidenceClass, IndexedImport};

pub(crate) struct SyntaxExtraction {
    pub(crate) symbols: Vec<IndexedSymbol>,
    pub(crate) imports: Vec<IndexedImport>,
}

pub(crate) fn extract_syntax_facts_for_file(
    relative_path: &str,
    language: &str,
    contents: &str,
    available_files: &BTreeSet<String>,
) -> Option<SyntaxExtraction> {
    let mut parser = Parser::new();
    configure_parser(&mut parser, language)?;
    let tree = parser.parse(contents, None)?;
    let source = contents.as_bytes();
    let skip_ranges = if language == "rust" {
        rust_cfg_test_skip_line_ranges(contents)
    } else {
        Vec::new()
    };

    let mut extraction = SyntaxExtraction {
        symbols: Vec::new(),
        imports: Vec::new(),
    };
    walk_tree(tree.root_node(), &mut |node| {
        if let Some(symbol) =
            syntax_symbol_for_node(node, source, relative_path, language, &skip_ranges)
        {
            extraction.symbols.push(symbol);
        }

        if let Some(import) =
            syntax_import_for_node(node, source, relative_path, language, available_files)
        {
            extraction.imports.push(import);
        }
    });

    Some(extraction)
}

fn configure_parser(parser: &mut Parser, language: &str) -> Option<()> {
    let configured = match language {
        "rust" => parser.set_language(&tree_sitter_rust::LANGUAGE.into()),
        "python" => parser.set_language(&tree_sitter_python::LANGUAGE.into()),
        "javascript" => parser.set_language(&tree_sitter_javascript::LANGUAGE.into()),
        "typescript" => parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        _ => return None,
    };

    configured.ok()
}

fn walk_tree<'tree>(node: Node<'tree>, visitor: &mut impl FnMut(Node<'tree>)) {
    visitor(node);

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_tree(child, visitor);
    }
}

fn syntax_symbol_for_node(
    node: Node<'_>,
    source: &[u8],
    relative_path: &str,
    language: &str,
    skip_ranges: &[(u32, u32)],
) -> Option<IndexedSymbol> {
    if language == "rust" && line_is_skipped(line_number_for_node(node), skip_ranges) {
        return None;
    }

    let kind = match language {
        "rust" => match node.kind() {
            "function_item" => SymbolKind::Function,
            "struct_item" => SymbolKind::Struct,
            "enum_item" => SymbolKind::Enum,
            "trait_item" => SymbolKind::Trait,
            "type_item" => SymbolKind::TypeAlias,
            "const_item" | "static_item" => SymbolKind::Constant,
            "mod_item" => SymbolKind::Module,
            _ => return None,
        },
        "python" => match node.kind() {
            "function_definition" => SymbolKind::Function,
            "class_definition" => SymbolKind::Class,
            _ => return None,
        },
        "javascript" | "typescript" => match node.kind() {
            "function_declaration" => SymbolKind::Function,
            "class_declaration" => SymbolKind::Class,
            "interface_declaration" => SymbolKind::Interface,
            "type_alias_declaration" => SymbolKind::TypeAlias,
            "enum_declaration" => SymbolKind::Enum,
            _ => return None,
        },
        _ => return None,
    };
    let name = node
        .child_by_field_name("name")?
        .utf8_text(source)
        .ok()?
        .trim();
    if name.is_empty() {
        return None;
    }

    Some(IndexedSymbol::new(
        name,
        kind,
        relative_path,
        language,
        line_number_for_node(node),
        SymbolEvidenceClass::SyntaxConfirmed,
    ))
}

fn syntax_import_for_node(
    node: Node<'_>,
    source: &[u8],
    relative_path: &str,
    language: &str,
    available_files: &BTreeSet<String>,
) -> Option<IndexedImport> {
    let text = node.utf8_text(source).ok()?.trim();
    let line_number = line_number_for_node(node);

    let (kind, import_spec, resolved_path) = match language {
        "rust" => match node.kind() {
            "mod_item" | "use_declaration" => {
                extract_rust_import(relative_path, text, available_files)?
            }
            _ => return None,
        },
        "python" => match node.kind() {
            "import_from_statement" => extract_python_import(relative_path, text, available_files)?,
            _ => return None,
        },
        "javascript" | "typescript" => match node.kind() {
            "import_statement" | "call_expression" => {
                extract_typescript_import(relative_path, text, available_files)?
            }
            _ => return None,
        },
        _ => return None,
    };

    Some(IndexedImport::new(
        relative_path,
        import_spec,
        line_number,
        language,
        kind,
        resolved_path,
        ImportEvidenceClass::SyntaxConfirmed,
    ))
}

fn line_number_for_node(node: Node<'_>) -> u32 {
    u32::try_from(node.start_position().row + 1).unwrap_or(u32::MAX)
}

fn line_is_skipped(line_number: u32, skip_ranges: &[(u32, u32)]) -> bool {
    skip_ranges
        .iter()
        .any(|(start, end)| (*start..=*end).contains(&line_number))
}

fn rust_cfg_test_skip_line_ranges(contents: &str) -> Vec<(u32, u32)> {
    let mut ranges = Vec::new();
    let mut cfg_test_pending = false;
    let mut brace_depth = 0_usize;
    let mut skip_start_line = None;
    let mut skip_test_module_depth = None;

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        let line_number = u32::try_from(line_index + 1).unwrap_or(u32::MAX);
        let next_brace_depth = update_brace_depth(brace_depth, trimmed);

        if let Some(skip_depth) = skip_test_module_depth {
            brace_depth = next_brace_depth;
            if brace_depth == skip_depth {
                if let Some(start_line) = skip_start_line.take() {
                    ranges.push((start_line, line_number));
                }
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
            skip_start_line = Some(line_number);
            skip_test_module_depth = Some(brace_depth);
            cfg_test_pending = false;
            brace_depth = next_brace_depth;
            if brace_depth == skip_test_module_depth.unwrap_or_default() {
                if let Some(start_line) = skip_start_line.take() {
                    ranges.push((start_line, line_number));
                }
                skip_test_module_depth = None;
            }
            continue;
        }

        if cfg_test_pending && !trimmed.is_empty() && !trimmed.starts_with("#[") {
            cfg_test_pending = false;
        }

        brace_depth = next_brace_depth;
    }

    ranges
}

fn update_brace_depth(current_depth: usize, line: &str) -> usize {
    let opening = line.chars().filter(|character| *character == '{').count();
    let closing = line.chars().filter(|character| *character == '}').count();

    current_depth + opening - closing.min(current_depth + opening)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use crate::{ImportKind, SymbolEvidenceClass, SymbolKind};

    use super::extract_syntax_facts_for_file;

    #[test]
    fn syntax_extraction_ignores_string_literal_false_positives() {
        let extraction = extract_syntax_facts_for_file(
            "src/lib.rs",
            "rust",
            r#"
pub struct RepositoryScanner {}

const FAKE: &str = "pub struct NotReal {}";
"#,
            &BTreeSet::from(["src/lib.rs".to_string()]),
        )
        .unwrap_or_else(|| panic!("expected syntax extraction"));

        assert_eq!(extraction.symbols.len(), 2);
        assert_eq!(extraction.symbols[0].kind, SymbolKind::Struct);
        assert_eq!(
            extraction.symbols[0].evidence_class,
            SymbolEvidenceClass::SyntaxConfirmed
        );
    }

    #[test]
    fn syntax_extraction_collects_typescript_imports_and_symbols() {
        let extraction = extract_syntax_facts_for_file(
            "src/module.ts",
            "typescript",
            r#"
import type { SnapshotBinding } from "./contracts.js";
export function scan(): void {}
const local = require("./helpers");
"#,
            &BTreeSet::from([
                "src/module.ts".to_string(),
                "src/contracts.ts".to_string(),
                "src/helpers.ts".to_string(),
            ]),
        )
        .unwrap_or_else(|| panic!("expected syntax extraction"));

        assert_eq!(extraction.symbols.len(), 1);
        assert_eq!(extraction.symbols[0].kind, SymbolKind::Function);
        assert_eq!(extraction.imports.len(), 2);
        assert_eq!(extraction.imports[0].kind, ImportKind::JSImport);
        assert_eq!(extraction.imports[1].kind, ImportKind::Require);
    }
}
