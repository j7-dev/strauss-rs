use std::path::{Path, PathBuf};

/// A file discovered in the project, either a project source file or a vendor file.
///
/// Mirrors the PHP `File` class. Files without a dependency are project source files
/// and vendor/composer autoload files.
#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    /// The absolute path to the file on disk.
    pub source_absolute_path: PathBuf,

    /// The path relative to the vendor directory, e.g. `vendor/package/src/Foo.php`.
    pub vendor_relative_path: String,

    /// The absolute path where this file should be copied to (the target directory).
    /// Falls back to `source_absolute_path` if not set.
    pub absolute_target_path: PathBuf,

    /// Whether this file should be copied to the target directory.
    pub do_copy: bool,

    /// Whether symbols discovered in this file should be prefixed (i.e. class definitions etc.).
    pub do_prefix: bool,

    /// Whether this file should be deleted from the source directory.
    /// `None` means defer to the package's `is_delete` setting.
    pub do_delete: Option<bool>,

    /// Whether this file has been deleted.
    pub did_delete: bool,

    /// Whether this file has been updated (had replacements applied).
    pub did_update: bool,

    /// Whether this file is referenced in an autoloader configuration.
    pub is_autoloaded: bool,

    /// The autoloader types that include this file, e.g. `["psr-4", "classmap", "files"]`.
    pub autoloader_types: Vec<String>,

    /// The Composer package name this file belongs to, if any.
    pub package_name: Option<String>,

    /// The path relative to the package root directory.
    pub package_relative_path: Option<String>,

    /// Keys of symbols discovered in this file (original fully-qualified names).
    pub discovered_symbols: Vec<String>,
}

impl DiscoveredFile {
    /// Create a new file without a dependency (project source or autoload file).
    ///
    /// Mirrors the PHP `File` constructor.
    pub fn new(source_absolute_path: PathBuf, vendor_relative_path: String) -> Self {
        let absolute_target_path = source_absolute_path.clone();
        Self {
            source_absolute_path,
            vendor_relative_path,
            absolute_target_path,
            do_copy: true,
            do_prefix: false,
            do_delete: Some(false),
            did_delete: false,
            did_update: false,
            is_autoloaded: false,
            autoloader_types: Vec::new(),
            package_name: None,
            package_relative_path: None,
            discovered_symbols: Vec::new(),
        }
    }

    /// Create a new file that belongs to a dependency package.
    ///
    /// Mirrors the PHP `FileWithDependency` constructor.
    pub fn new_with_dependency(
        source_absolute_path: PathBuf,
        vendor_relative_path: String,
        package_name: String,
        package_absolute_path: &Path,
    ) -> Self {
        let absolute_target_path = source_absolute_path.clone();

        // Compute the package-relative path by stripping the package absolute path prefix.
        let package_relative_path = source_absolute_path.to_str().and_then(|src| {
            let pkg = package_absolute_path.to_str()?;
            src.strip_prefix(pkg).map(|s| s.to_string())
        });

        // Normalize vendor_relative_path: strip leading slashes
        let vendor_relative_path = vendor_relative_path
            .trim_start_matches('/')
            .trim_start_matches('\\')
            .to_string();

        Self {
            source_absolute_path,
            vendor_relative_path,
            absolute_target_path,
            do_copy: true,
            do_prefix: false,
            do_delete: None, // defer to package setting
            did_delete: false,
            did_update: false,
            is_autoloaded: false,
            autoloader_types: Vec::new(),
            package_name: Some(package_name),
            package_relative_path,
            discovered_symbols: Vec::new(),
        }
    }

    /// Check if this file has a `.php` extension.
    pub fn is_php_file(&self) -> bool {
        self.source_absolute_path
            .extension()
            .map(|ext| ext.eq_ignore_ascii_case("php"))
            .unwrap_or(false)
    }

    /// Check if this file is included via a `files` autoloader entry.
    ///
    /// Mirrors the PHP `isFilesAutoloaderFile()` method.
    pub fn is_files_autoloader_file(&self) -> bool {
        self.autoloader_types.contains(&"files".to_string())
    }

    /// Get the source path as a string reference.
    pub fn get_source_path(&self) -> &str {
        self.source_absolute_path.to_str().unwrap_or_default()
    }

    /// Get the effective target path for this file.
    ///
    /// Returns `absolute_target_path` if set, otherwise falls back to `source_absolute_path`.
    pub fn get_absolute_target_path(&self) -> &Path {
        &self.absolute_target_path
    }

    /// Add an autoloader type entry (deduplicating).
    pub fn add_autoloader(&mut self, autoloader_type: String) {
        if !self.autoloader_types.contains(&autoloader_type) {
            self.autoloader_types.push(autoloader_type);
        }
    }

    /// Add a discovered symbol key to this file (deduplicating).
    pub fn add_discovered_symbol(&mut self, symbol_key: String) {
        if !self.discovered_symbols.contains(&symbol_key) {
            self.discovered_symbols.push(symbol_key);
        }
    }

    /// Whether this file belongs to a dependency package.
    pub fn has_dependency(&self) -> bool {
        self.package_name.is_some()
    }

    /// Whether this file should be deleted.
    ///
    /// For dependency files where `do_delete` is `None`, the caller should
    /// check the package-level `is_delete` setting.
    pub fn is_do_delete(&self) -> bool {
        self.do_delete.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_file() {
        let file = DiscoveredFile::new(
            PathBuf::from("/project/vendor/autoload.php"),
            "vendor/autoload.php".to_string(),
        );
        assert!(file.do_copy);
        assert!(!file.do_prefix);
        assert_eq!(file.do_delete, Some(false));
        assert!(!file.did_delete);
        assert!(!file.did_update);
        assert!(!file.is_autoloaded);
        assert!(file.autoloader_types.is_empty());
        assert!(file.package_name.is_none());
        assert!(file.discovered_symbols.is_empty());
    }

    #[test]
    fn test_new_with_dependency() {
        let file = DiscoveredFile::new_with_dependency(
            PathBuf::from("/project/vendor/psr/log/src/LoggerInterface.php"),
            "psr/log/src/LoggerInterface.php".to_string(),
            "psr/log".to_string(),
            Path::new("/project/vendor/psr/log"),
        );
        assert_eq!(file.package_name.as_deref(), Some("psr/log"));
        assert_eq!(file.do_delete, None); // defer to package
        assert!(file.has_dependency());
    }

    #[test]
    fn test_is_php_file() {
        let php = DiscoveredFile::new(
            PathBuf::from("/project/src/Foo.php"),
            "src/Foo.php".to_string(),
        );
        assert!(php.is_php_file());

        let json = DiscoveredFile::new(
            PathBuf::from("/project/composer.json"),
            "composer.json".to_string(),
        );
        assert!(!json.is_php_file());

        let upper = DiscoveredFile::new(
            PathBuf::from("/project/src/Bar.PHP"),
            "src/Bar.PHP".to_string(),
        );
        assert!(upper.is_php_file());
    }

    #[test]
    fn test_is_files_autoloader_file() {
        let mut file = DiscoveredFile::new(
            PathBuf::from("/project/vendor/pkg/helpers.php"),
            "pkg/helpers.php".to_string(),
        );
        assert!(!file.is_files_autoloader_file());

        file.add_autoloader("files".to_string());
        assert!(file.is_files_autoloader_file());
    }

    #[test]
    fn test_add_autoloader_dedup() {
        let mut file = DiscoveredFile::new(
            PathBuf::from("/project/vendor/pkg/Foo.php"),
            "pkg/Foo.php".to_string(),
        );
        file.add_autoloader("psr-4".to_string());
        file.add_autoloader("psr-4".to_string());
        file.add_autoloader("classmap".to_string());
        assert_eq!(file.autoloader_types.len(), 2);
    }

    #[test]
    fn test_add_discovered_symbol_dedup() {
        let mut file = DiscoveredFile::new(
            PathBuf::from("/project/vendor/pkg/Foo.php"),
            "pkg/Foo.php".to_string(),
        );
        file.add_discovered_symbol("Vendor\\Foo".to_string());
        file.add_discovered_symbol("Vendor\\Foo".to_string());
        assert_eq!(file.discovered_symbols.len(), 1);
    }

    #[test]
    fn test_vendor_relative_path_trimmed() {
        let file = DiscoveredFile::new_with_dependency(
            PathBuf::from("/project/vendor/psr/log/src/Logger.php"),
            "/psr/log/src/Logger.php".to_string(),
            "psr/log".to_string(),
            Path::new("/project/vendor/psr/log"),
        );
        assert_eq!(file.vendor_relative_path, "psr/log/src/Logger.php");
    }

    #[test]
    fn test_is_do_delete_defaults() {
        let file = DiscoveredFile::new(
            PathBuf::from("/project/src/Foo.php"),
            "src/Foo.php".to_string(),
        );
        // Non-dependency file defaults to false
        assert!(!file.is_do_delete());

        let dep_file = DiscoveredFile::new_with_dependency(
            PathBuf::from("/project/vendor/pkg/Foo.php"),
            "pkg/Foo.php".to_string(),
            "some/package".to_string(),
            Path::new("/project/vendor/pkg"),
        );
        // Dependency file with None defaults to false (caller should check package setting)
        assert!(!dep_file.is_do_delete());
    }
}
