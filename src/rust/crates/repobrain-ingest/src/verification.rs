use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{IndexedFile, IngestError};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationTargetKind {
    Test,
    Typecheck,
    Quality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationTargetPriority {
    Required,
    Recommended,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedVerificationTarget {
    pub label: String,
    pub scope_root: String,
    pub working_directory: String,
    pub command: Vec<String>,
    pub kind: VerificationTargetKind,
    pub priority: VerificationTargetPriority,
}

pub(crate) fn extract_verification_targets(
    repo_root: &Path,
    files: &[IndexedFile],
) -> Result<Vec<IndexedVerificationTarget>, IngestError> {
    let mut targets = Vec::new();

    for file in files {
        match file.relative_path.as_str() {
            "Cargo.toml" => {
                let contents = std::fs::read_to_string(repo_root.join(&file.relative_path))
                    .map_err(|source| {
                        IngestError::io(repo_root.join(&file.relative_path), source)
                    })?;
                targets.extend(extract_workspace_targets(&contents, files));
                if let Some(package_targets) =
                    extract_cargo_package_targets(&file.relative_path, &contents)
                {
                    targets.extend(package_targets);
                }
            }
            "package.json" => {
                let contents = std::fs::read_to_string(repo_root.join(&file.relative_path))
                    .map_err(|source| {
                        IngestError::io(repo_root.join(&file.relative_path), source)
                    })?;
                targets.extend(extract_root_package_targets(&contents)?);
            }
            path if path.ends_with("/Cargo.toml") => {
                let contents = std::fs::read_to_string(repo_root.join(&file.relative_path))
                    .map_err(|source| {
                        IngestError::io(repo_root.join(&file.relative_path), source)
                    })?;
                if let Some(target) = extract_cargo_package_targets(&file.relative_path, &contents)
                {
                    targets.extend(target);
                }
            }
            path if path.ends_with("/package.json") => {
                let contents = std::fs::read_to_string(repo_root.join(&file.relative_path))
                    .map_err(|source| {
                        IngestError::io(repo_root.join(&file.relative_path), source)
                    })?;
                targets.extend(extract_scoped_package_targets(
                    &file.relative_path,
                    &contents,
                )?);
            }
            path if path.ends_with("/pyproject.toml") => {
                let contents = std::fs::read_to_string(repo_root.join(&file.relative_path))
                    .map_err(|source| {
                        IngestError::io(repo_root.join(&file.relative_path), source)
                    })?;
                targets.extend(extract_python_package_targets(
                    &file.relative_path,
                    &contents,
                    files,
                ));
            }
            _ => {}
        }
    }

    Ok(targets)
}

fn extract_workspace_targets(
    contents: &str,
    files: &[IndexedFile],
) -> Vec<IndexedVerificationTarget> {
    let mut targets = Vec::new();
    if !contents.lines().any(|line| line.trim() == "[workspace]") {
        return targets;
    }

    if files
        .iter()
        .any(|file| file.relative_path == "src/rust/xtask/Cargo.toml")
    {
        targets.push(IndexedVerificationTarget {
            label: "repo-native quality".to_string(),
            scope_root: String::new(),
            working_directory: ".".to_string(),
            command: vec![
                "cargo".to_string(),
                "xtask".to_string(),
                "quality".to_string(),
            ],
            kind: VerificationTargetKind::Quality,
            priority: VerificationTargetPriority::Recommended,
        });
        targets.push(IndexedVerificationTarget {
            label: "repo-native full check".to_string(),
            scope_root: String::new(),
            working_directory: ".".to_string(),
            command: vec![
                "cargo".to_string(),
                "xtask".to_string(),
                "check".to_string(),
            ],
            kind: VerificationTargetKind::Quality,
            priority: VerificationTargetPriority::Recommended,
        });
    }

    targets
}

fn extract_root_package_targets(
    contents: &str,
) -> Result<Vec<IndexedVerificationTarget>, IngestError> {
    let value =
        serde_json::from_str::<Value>(contents).map_err(|source| IngestError::Deserialize {
            path: Path::new("package.json").to_path_buf(),
            source,
        })?;
    let mut targets = Vec::new();

    if has_package_script(&value, "quality:check") {
        targets.push(IndexedVerificationTarget {
            label: "root quality check".to_string(),
            scope_root: String::new(),
            working_directory: ".".to_string(),
            command: vec!["pnpm".to_string(), "quality:check".to_string()],
            kind: VerificationTargetKind::Quality,
            priority: VerificationTargetPriority::Recommended,
        });
    }

    Ok(targets)
}

fn extract_cargo_package_targets(
    manifest_path: &str,
    contents: &str,
) -> Option<Vec<IndexedVerificationTarget>> {
    let package_name = parse_toml_section_string(contents, "package", "name")?;
    let scope_root = parent_relative_path(manifest_path);
    let working_directory = display_working_directory(&scope_root);

    Some(vec![
        IndexedVerificationTarget {
            label: format!("cargo tests for {package_name}"),
            scope_root: scope_root.clone(),
            working_directory: working_directory.clone(),
            command: vec!["cargo".to_string(), "test".to_string()],
            kind: VerificationTargetKind::Test,
            priority: VerificationTargetPriority::Required,
        },
        IndexedVerificationTarget {
            label: format!("cargo check for {package_name}"),
            scope_root,
            working_directory,
            command: vec!["cargo".to_string(), "check".to_string()],
            kind: VerificationTargetKind::Typecheck,
            priority: VerificationTargetPriority::Recommended,
        },
    ])
}

fn extract_scoped_package_targets(
    manifest_path: &str,
    contents: &str,
) -> Result<Vec<IndexedVerificationTarget>, IngestError> {
    let value =
        serde_json::from_str::<Value>(contents).map_err(|source| IngestError::Deserialize {
            path: Path::new(manifest_path).to_path_buf(),
            source,
        })?;
    let mut targets = Vec::new();
    let Some(package_name) = value.get("name").and_then(Value::as_str) else {
        return Ok(targets);
    };
    let scope_root = parent_relative_path(manifest_path);
    let working_directory = display_working_directory(&scope_root);

    if has_package_script(&value, "typecheck") {
        targets.push(IndexedVerificationTarget {
            label: format!("typecheck for {package_name}"),
            scope_root,
            working_directory,
            command: vec!["pnpm".to_string(), "typecheck".to_string()],
            kind: VerificationTargetKind::Typecheck,
            priority: VerificationTargetPriority::Required,
        });
    }

    Ok(targets)
}

fn extract_python_package_targets(
    manifest_path: &str,
    contents: &str,
    files: &[IndexedFile],
) -> Vec<IndexedVerificationTarget> {
    let Some(package_name) = parse_toml_section_string(contents, "project", "name") else {
        return Vec::new();
    };
    let scope_root = parent_relative_path(manifest_path);
    let working_directory = display_working_directory(&scope_root);
    let has_tests = has_file_under_scope(files, &scope_root, "tests");
    let has_src = has_file_under_scope(files, &scope_root, "src");
    let mut targets = Vec::new();

    if has_tests {
        targets.push(IndexedVerificationTarget {
            label: format!("unit tests for {package_name}"),
            scope_root: scope_root.clone(),
            working_directory: working_directory.clone(),
            command: vec![
                "python".to_string(),
                "-m".to_string(),
                "unittest".to_string(),
                "discover".to_string(),
                "tests".to_string(),
            ],
            kind: VerificationTargetKind::Test,
            priority: VerificationTargetPriority::Required,
        });
    }

    if contents.contains("[tool.mypy]") && has_src {
        let mut command = vec![
            "python".to_string(),
            "-m".to_string(),
            "mypy".to_string(),
            "--config-file".to_string(),
            "pyproject.toml".to_string(),
            "src".to_string(),
        ];
        if has_tests {
            command.push("tests".to_string());
        }
        targets.push(IndexedVerificationTarget {
            label: format!("mypy for {package_name}"),
            scope_root: scope_root.clone(),
            working_directory: working_directory.clone(),
            command,
            kind: VerificationTargetKind::Typecheck,
            priority: VerificationTargetPriority::Recommended,
        });
    }

    if contents.contains("[tool.ruff]") && has_src {
        let mut command = vec![
            "python".to_string(),
            "-m".to_string(),
            "ruff".to_string(),
            "check".to_string(),
            "src".to_string(),
        ];
        if has_tests {
            command.push("tests".to_string());
        }
        targets.push(IndexedVerificationTarget {
            label: format!("ruff for {package_name}"),
            scope_root,
            working_directory,
            command,
            kind: VerificationTargetKind::Quality,
            priority: VerificationTargetPriority::Recommended,
        });
    }

    targets
}

fn parse_toml_section_string(contents: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_section = trimmed == format!("[{section}]");
            continue;
        }

        if !in_section {
            continue;
        }

        let prefix = format!("{key} = ");
        if let Some(value) = trimmed.strip_prefix(&prefix) {
            return parse_toml_string_literal(value);
        }
    }

    None
}

fn parse_toml_string_literal(value: &str) -> Option<String> {
    let value = value.trim();
    let stripped = value.strip_prefix('"')?;
    let end_index = stripped.find('"')?;

    Some(stripped[..end_index].to_string())
}

fn has_package_script(value: &Value, script_name: &str) -> bool {
    value
        .get("scripts")
        .and_then(Value::as_object)
        .is_some_and(|scripts| scripts.get(script_name).and_then(Value::as_str).is_some())
}

fn parent_relative_path(relative_path: &str) -> String {
    Path::new(relative_path)
        .parent()
        .map(|parent| parent.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

fn display_working_directory(scope_root: &str) -> String {
    if scope_root.is_empty() {
        ".".to_string()
    } else {
        scope_root.to_string()
    }
}

fn has_file_under_scope(files: &[IndexedFile], scope_root: &str, child_dir: &str) -> bool {
    let prefix = if scope_root.is_empty() {
        format!("{child_dir}/")
    } else {
        format!("{scope_root}/{child_dir}/")
    };

    files
        .iter()
        .any(|file| file.relative_path.starts_with(&prefix))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        display_working_directory, has_package_script, parent_relative_path,
        parse_toml_section_string,
    };

    #[test]
    fn parses_string_values_from_named_toml_sections() {
        let contents = r#"
[package]
name = "repobrain-ingest"
"#;

        assert_eq!(
            parse_toml_section_string(contents, "package", "name"),
            Some("repobrain-ingest".to_string())
        );
    }

    #[test]
    fn root_working_directory_displays_as_dot() {
        assert_eq!(display_working_directory(""), ".".to_string());
        assert_eq!(
            parent_relative_path("src/rust/crates/repobrain-ingest/Cargo.toml"),
            "src/rust/crates/repobrain-ingest".to_string()
        );
    }

    #[test]
    fn package_script_detection_checks_scripts_object() {
        let value = json!({
            "scripts": {
                "typecheck": "tsc -p tsconfig.json --noEmit"
            }
        });

        assert!(has_package_script(&value, "typecheck"));
        assert!(!has_package_script(&value, "test"));
    }
}
