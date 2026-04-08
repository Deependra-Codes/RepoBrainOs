use super::{BTreeSet, Path, PathBuf, fs};

pub(super) fn theorem_collect_llvm_artifact_paths(
    cargo_stdout: &[u8],
    shared_target_dir: &Path,
    workspace_root: &Path,
    manifest_path: &str,
) -> Vec<PathBuf> {
    let mut paths = BTreeSet::<PathBuf>::new();
    let manifest_candidate = PathBuf::from(manifest_path);
    let manifest_path = if manifest_candidate.is_absolute() {
        manifest_candidate
    } else {
        workspace_root.join(manifest_candidate)
    };
    let normalized_manifest_path = manifest_path
        .to_string_lossy()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let mut target_names = BTreeSet::<String>::new();

    for line in String::from_utf8_lossy(cargo_stdout).lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if value.get("reason").and_then(|field| field.as_str()) != Some("compiler-artifact") {
            continue;
        }
        let Some(artifact_manifest_path) =
            value.get("manifest_path").and_then(|field| field.as_str())
        else {
            continue;
        };
        let normalized_artifact_manifest_path = artifact_manifest_path
            .replace('\\', "/")
            .to_ascii_lowercase();
        if normalized_artifact_manifest_path != normalized_manifest_path {
            continue;
        }
        if value
            .get("target")
            .and_then(|field| field.get("kind"))
            .and_then(|field| field.as_array())
            .is_some_and(|kinds| {
                kinds
                    .iter()
                    .any(|kind| kind.as_str() == Some("custom-build"))
            })
        {
            continue;
        }
        if let Some(target_name) = value
            .get("target")
            .and_then(|field| field.get("name"))
            .and_then(|field| field.as_str())
        {
            target_names.insert(theorem_normalize_llvm_target_name(target_name));
        }
        let Some(filenames) = value.get("filenames").and_then(|field| field.as_array()) else {
            continue;
        };
        for filename in filenames {
            let Some(path) = filename.as_str() else {
                continue;
            };
            if !Path::new(path)
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ll"))
            {
                continue;
            }
            let candidate = PathBuf::from(path);
            let absolute = if candidate.is_absolute() {
                candidate
            } else {
                workspace_root.join(candidate)
            };
            paths.insert(absolute);
        }
    }

    if paths.is_empty() {
        paths.extend(theorem_collect_llvm_target_dir_paths(
            shared_target_dir,
            &target_names,
        ));
    }

    paths.into_iter().collect()
}

fn theorem_collect_llvm_target_dir_paths(
    shared_target_dir: &Path,
    target_names: &BTreeSet<String>,
) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::<PathBuf>::new();
    let mut pending = vec![shared_target_dir.to_path_buf()];

    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if theorem_llvm_path_is_incremental(&path) {
                    continue;
                }
                pending.push(path);
                continue;
            }
            if !path
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("ll"))
            {
                continue;
            }
            if theorem_llvm_path_is_incremental(&path)
                || !theorem_llvm_path_matches_target(&path, target_names)
            {
                continue;
            }
            paths.insert(path);
        }
    }

    paths
}

fn theorem_llvm_path_is_incremental(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("incremental")
    })
}

fn theorem_llvm_path_matches_target(path: &Path, target_names: &BTreeSet<String>) -> bool {
    if target_names.is_empty() {
        return true;
    }
    let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
        return false;
    };
    let normalized = theorem_normalize_llvm_target_name(stem);
    let normalized = normalized.strip_prefix("lib").unwrap_or(&normalized);
    target_names.iter().any(|target_name| {
        normalized == target_name || normalized.starts_with(&format!("{target_name}_"))
    })
}

fn theorem_normalize_llvm_target_name(name: &str) -> String {
    name.replace('-', "_").to_ascii_lowercase()
}
