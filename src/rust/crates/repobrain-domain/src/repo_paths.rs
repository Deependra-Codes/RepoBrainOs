use std::io;
use std::path::{Path, PathBuf};

/// Canonicalizes a repo root and normalizes platform-specific path details so
/// callers share one stable representation.
///
/// # Errors
///
/// Returns an error when the provided path does not exist, cannot be resolved,
/// or cannot be canonicalized by the operating system.
pub fn canonicalize_repo_root(path: &Path) -> io::Result<PathBuf> {
    path.canonicalize()
        .map(|canonical_path| normalize_repo_root(&canonical_path))
}

/// Normalizes repo-root display details without widening into a general path
/// utility surface.
#[must_use]
pub fn normalize_repo_root(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let raw = path.to_string_lossy();
        if let Some(stripped) = raw.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }

    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{canonicalize_repo_root, normalize_repo_root};

    #[test]
    fn canonicalize_repo_root_returns_normalized_absolute_path() {
        let canonical_current_dir = match Path::new(".").canonicalize() {
            Ok(path) => path,
            Err(error) => panic!("failed to canonicalize current directory: {error}"),
        };
        let resolved = match canonicalize_repo_root(Path::new(".")) {
            Ok(path) => path,
            Err(error) => panic!("failed to resolve repo root: {error}"),
        };

        assert!(resolved.is_absolute());
        assert_eq!(resolved, normalize_repo_root(&canonical_current_dir));
    }

    #[cfg(windows)]
    #[test]
    fn normalize_repo_root_strips_windows_verbatim_prefix() {
        let normalized = normalize_repo_root(Path::new(r"\\?\D:\RepoBrainOS"));

        assert_eq!(normalized, PathBuf::from(r"D:\RepoBrainOS"));
    }

    #[cfg(not(windows))]
    #[test]
    fn normalize_repo_root_is_identity_for_non_windows_paths() {
        let normalized = normalize_repo_root(Path::new("/tmp/repobrain"));

        assert_eq!(normalized, PathBuf::from("/tmp/repobrain"));
    }
}
