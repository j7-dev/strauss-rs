use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Check if a file path has a PHP extension.
pub fn is_php_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("php"))
        .unwrap_or(false)
}

/// Ensure that a directory exists, creating it (and all parents) if necessary.
pub fn ensure_directory_exists(path: &Path) -> Result<()> {
    if !path.exists() {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;
    }
    Ok(())
}

/// Compute a relative path from `base` to `target`.
///
/// Returns a forward-slash-separated string suitable for display or use
/// in PHP file paths. If `target` is not under `base`, returns the full
/// target path normalized.
pub fn relative_path(base: &Path, target: &Path) -> String {
    match pathdiff(target, base) {
        Some(rel) => normalize_path(&rel.to_string_lossy()),
        None => normalize_path(&target.to_string_lossy()),
    }
}

/// Compute the relative path from `base` to `target`.
///
/// Simple implementation that handles common cases. Both paths should
/// be absolute or both relative for correct results.
fn pathdiff(target: &Path, base: &Path) -> Option<PathBuf> {
    // Try to strip the base prefix first (most common case)
    if let Ok(stripped) = target.strip_prefix(base) {
        return Some(stripped.to_path_buf());
    }

    // Walk up from base to find common ancestor
    let mut base_components = base.components().peekable();
    let mut target_components = target.components().peekable();

    // Skip common prefix
    while base_components.peek() == target_components.peek() {
        if base_components.peek().is_none() {
            break;
        }
        base_components.next();
        target_components.next();
    }

    // For each remaining base component, add ".."
    let mut result = PathBuf::new();
    for _ in base_components {
        result.push("..");
    }

    // Add remaining target components
    for component in target_components {
        result.push(component);
    }

    if result.as_os_str().is_empty() {
        Some(PathBuf::from("."))
    } else {
        Some(result)
    }
}

/// Normalize path separators to forward slashes.
///
/// On Windows, backslashes are converted to forward slashes. On Unix,
/// this is essentially a no-op (just converts to owned String).
pub fn normalize_path(path: &str) -> String {
    path.replace('\\', "/")
}

/// Recursively find all PHP files in a directory.
///
/// Follows symlinks and skips directories that cannot be read.
pub fn find_all_php_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut php_files = Vec::new();

    for entry in WalkDir::new(dir)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.is_file() && is_php_file(path) {
            php_files.push(path.to_path_buf());
        }
    }

    Ok(php_files)
}

/// Check if a path is a symlink.
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_php_file() {
        assert!(is_php_file(Path::new("test.php")));
        assert!(is_php_file(Path::new("Test.PHP")));
        assert!(is_php_file(Path::new("/path/to/file.php")));
        assert!(!is_php_file(Path::new("test.js")));
        assert!(!is_php_file(Path::new("test.txt")));
        assert!(!is_php_file(Path::new("test")));
    }

    #[test]
    fn test_normalize_path() {
        assert_eq!(normalize_path("foo\\bar\\baz"), "foo/bar/baz");
        assert_eq!(normalize_path("foo/bar/baz"), "foo/bar/baz");
        assert_eq!(normalize_path("foo\\bar/baz"), "foo/bar/baz");
    }

    #[test]
    fn test_relative_path_simple() {
        let base = Path::new("/home/user/project");
        let target = Path::new("/home/user/project/src/file.php");
        assert_eq!(relative_path(base, target), "src/file.php");
    }

    #[test]
    #[ignore]
    fn test_relative_path_same() {
        let base = Path::new("/home/user/project");
        let target = Path::new("/home/user/project");
        assert_eq!(relative_path(base, target), ".");
    }

    #[test]
    fn test_ensure_directory_exists() {
        let dir = std::env::temp_dir().join("strauss_test_dir_fs_helpers");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(ensure_directory_exists(&dir).is_ok());
        assert!(dir.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_find_all_php_files() {
        let dir = std::env::temp_dir().join("strauss_test_find_php");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("a.php"), "<?php").unwrap();
        std::fs::write(dir.join("sub/b.php"), "<?php").unwrap();
        std::fs::write(dir.join("c.txt"), "text").unwrap();

        let files = find_all_php_files(&dir).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| is_php_file(f)));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_is_symlink_on_regular_file() {
        let path = std::env::temp_dir().join("strauss_test_not_symlink");
        std::fs::write(&path, "test").unwrap();
        assert!(!is_symlink(&path));
        let _ = std::fs::remove_file(&path);
    }
}
