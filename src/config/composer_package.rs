use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use super::composer_json::{AutoloadConfig, ComposerJson, ComposerLockPackage, Psr4Value};

/// Flatten a PSR-4/PSR-0 map from Psr4Value (single or multiple paths) to Vec<String>.
fn flatten_psr4(psr: &HashMap<String, Psr4Value>) -> HashMap<String, Vec<String>> {
    psr.iter()
        .map(|(ns, val)| {
            let paths = match val {
                Psr4Value::Single(s) => vec![s.clone()],
                Psr4Value::Multiple(v) => v.clone(),
            };
            (ns.clone(), paths)
        })
        .collect()
}

/// Represents a single Composer package (dependency) with its metadata.
///
/// This is the Rust equivalent of the PHP `ComposerPackage` class. It holds
/// the package name, autoload configuration, dependency list, license, and
/// path information needed during the copy and prefix pipeline.
#[derive(Debug, Clone)]
pub struct ComposerPackage {
    /// Package name, e.g. "vendor/package-name"
    pub name: String,

    /// Version string from composer.lock
    pub version: Option<String>,

    /// Path relative to the vendor directory, e.g. "vendor/package-name"
    pub relative_path: Option<String>,

    /// Absolute path to the package directory on disk
    pub absolute_path: Option<PathBuf>,

    /// Autoload configuration (psr-4, psr-0, classmap, files)
    pub autoload: AutoloadConfig,

    /// Names of packages required by this package (excludes php and ext-*)
    pub requires_names: Vec<String>,

    /// License string (comma-separated if multiple)
    pub license: String,

    /// Whether this package should be copied to the target directory
    pub is_copy: bool,

    /// Whether this package has been copied
    pub did_copy: bool,

    /// Whether this package should be deleted from vendor
    pub is_delete: bool,

    /// Whether this package has been deleted from vendor
    pub did_delete: bool,

    // --- Flattened autoload fields for pipeline convenience ---
    /// PSR-4 namespace => directory mappings
    pub autoload_psr4: HashMap<String, Vec<String>>,
    /// PSR-0 namespace => directory mappings
    pub autoload_psr0: HashMap<String, Vec<String>>,
    /// Classmap directories
    pub autoload_classmap: Vec<String>,
    /// Files autoloader entries
    pub autoload_files: Vec<String>,
    /// Package requires (alias for requires_names)
    pub requires: Vec<String>,
}

impl ComposerPackage {
    /// Create a ComposerPackage from a composer.json file path.
    pub fn from_composer_json(path: &Path) -> anyhow::Result<Self> {
        let json = ComposerJson::from_path(path)?;
        let name = json.name.clone().unwrap_or_else(|| String::from("__root__"));
        let autoload = json.autoload();
        let requires_names = json.require_names();
        let license = json
            .license
            .as_ref()
            .map(|l| l.as_string())
            .unwrap_or_else(|| String::from("proprietary?"));

        let absolute_path = path
            .parent()
            .and_then(|p| p.canonicalize().ok());

        let autoload_psr4 = flatten_psr4(&autoload.psr4);
        let autoload_psr0 = flatten_psr4(&autoload.psr0);
        let autoload_classmap = autoload.classmap.clone();
        let autoload_files = autoload.files.clone();
        let requires = requires_names.clone();

        Ok(Self {
            name,
            version: json.version.clone(),
            relative_path: None,
            absolute_path,
            autoload,
            requires_names,
            license,
            is_copy: true,
            did_copy: false,
            is_delete: false,
            did_delete: false,
            autoload_psr4,
            autoload_psr0,
            autoload_classmap,
            autoload_files,
            requires,
        })
    }

    /// Create a ComposerPackage from a composer.lock package entry.
    pub fn from_lock_entry(
        entry: &ComposerLockPackage,
        vendor_dir: &Path,
        override_autoload: Option<&AutoloadConfig>,
    ) -> Self {
        let autoload = override_autoload
            .cloned()
            .unwrap_or_else(|| entry.autoload());

        let requires_names = entry.require_names();
        let license = entry.license_string();

        let pkg_dir = vendor_dir.join(&entry.name);
        let absolute_path = pkg_dir.canonicalize().ok();
        let relative_path = Some(entry.name.clone());

        let autoload_psr4 = flatten_psr4(&autoload.psr4);
        let autoload_psr0 = flatten_psr4(&autoload.psr0);
        let autoload_classmap = autoload.classmap.clone();
        let autoload_files = autoload.files.clone();
        let requires = requires_names.clone();

        Self {
            name: entry.name.clone(),
            version: entry.version.clone(),
            relative_path,
            absolute_path,
            autoload,
            requires_names,
            license,
            is_copy: true,
            did_copy: false,
            is_delete: false,
            did_delete: false,
            autoload_psr4,
            autoload_psr0,
            autoload_classmap,
            autoload_files,
            requires,
        }
    }

    /// Create a ComposerPackage from a raw JSON array (for virtual packages).
    pub fn from_json_value(
        name: &str,
        autoload: AutoloadConfig,
        requires: Vec<String>,
        license: &str,
    ) -> Self {
        let autoload_psr4 = flatten_psr4(&autoload.psr4);
        let autoload_psr0 = flatten_psr4(&autoload.psr0);
        let autoload_classmap = autoload.classmap.clone();
        let autoload_files = autoload.files.clone();
        let req = requires.clone();

        Self {
            name: name.to_string(),
            version: None,
            relative_path: Some(name.to_string()),
            absolute_path: None,
            autoload,
            requires_names: req,
            license: license.to_string(),
            is_copy: true,
            did_copy: false,
            is_delete: false,
            did_delete: false,
            autoload_psr4,
            autoload_psr0,
            autoload_classmap,
            autoload_files,
            requires,
        }
    }

    /// Create a ComposerPackage from a vendor directory (reads composer.json inside it).
    pub fn from_vendor_dir(
        package_dir: &Path,
        name: &str,
        override_autoload: &HashMap<String, Value>,
    ) -> anyhow::Result<Self> {
        let composer_json_path = package_dir.join("composer.json");
        let mut pkg = Self::from_composer_json(&composer_json_path)?;
        pkg.name = name.to_string();
        pkg.relative_path = Some(name.to_string());
        pkg.absolute_path = package_dir.canonicalize().ok();
        // Re-flatten after potential override
        pkg.autoload_psr4 = flatten_psr4(&pkg.autoload.psr4);
        pkg.autoload_psr0 = flatten_psr4(&pkg.autoload.psr0);
        pkg.autoload_classmap = pkg.autoload.classmap.clone();
        pkg.autoload_files = pkg.autoload.files.clone();
        pkg.requires = pkg.requires_names.clone();
        Ok(pkg)
    }

    /// Create a ComposerPackage from a composer.lock entry.
    pub fn from_lock_package(
        entry: &ComposerLockPackage,
        vendor_dir: &Path,
        override_autoload: &HashMap<String, Value>,
    ) -> anyhow::Result<Self> {
        Ok(Self::from_lock_entry(entry, vendor_dir, None))
    }

    // --- Getters ---

    /// The package name (e.g. "vendor/package-name").
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The package version, if known.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// The path relative to the vendor directory, with trailing slash.
    /// Returns None if not set (e.g. virtual packages).
    pub fn relative_path(&self) -> Option<String> {
        self.relative_path
            .as_ref()
            .map(|p| format!("{}/", p.trim_end_matches('/')))
    }

    /// The absolute filesystem path to the package directory.
    pub fn absolute_path(&self) -> Option<&Path> {
        self.absolute_path.as_deref()
    }

    /// The autoload configuration for this package.
    pub fn autoload(&self) -> &AutoloadConfig {
        &self.autoload
    }

    /// Replace the autoload configuration with an override.
    pub fn set_autoload(&mut self, autoload: AutoloadConfig) {
        self.autoload = autoload;
    }

    /// Names of packages this package requires (excluding php and ext-*).
    pub fn requires_names(&self) -> &[String] {
        &self.requires_names
    }

    /// The license string.
    pub fn license(&self) -> &str {
        &self.license
    }

    /// Whether this package should be copied to the target directory.
    pub fn is_copy(&self) -> bool {
        self.is_copy
    }

    /// Set whether this package should be copied.
    pub fn set_copy(&mut self, is_copy: bool) {
        self.is_copy = is_copy;
    }

    /// Whether this package has been copied.
    pub fn did_copy(&self) -> bool {
        self.did_copy
    }

    /// Mark this package as copied (or not).
    pub fn set_did_copy(&mut self, did_copy: bool) {
        self.did_copy = did_copy;
    }

    /// Whether this package should be deleted from vendor.
    pub fn is_delete(&self) -> bool {
        self.is_delete
    }

    /// Set whether this package should be deleted.
    pub fn set_delete(&mut self, is_delete: bool) {
        self.is_delete = is_delete;
    }

    /// Whether this package has been deleted.
    pub fn did_delete(&self) -> bool {
        self.did_delete
    }

    /// Mark this package as deleted (or not).
    pub fn set_did_delete(&mut self, did_delete: bool) {
        self.did_delete = did_delete;
    }

    /// Set the relative path.
    pub fn set_relative_path(&mut self, path: Option<String>) {
        self.relative_path = path;
    }

    /// Set the absolute path.
    pub fn set_absolute_path(&mut self, path: Option<PathBuf>) {
        self.absolute_path = path;
    }

    /// Get all PSR-4 namespace prefixes defined in this package's autoload.
    pub fn psr4_namespaces(&self) -> Vec<&str> {
        self.autoload.psr4.keys().map(|s| s.as_str()).collect()
    }

    /// Get all PSR-0 namespace prefixes defined in this package's autoload.
    pub fn psr0_namespaces(&self) -> Vec<&str> {
        self.autoload.psr0.keys().map(|s| s.as_str()).collect()
    }

    /// Get all classmap entries.
    pub fn classmap_entries(&self) -> &[String] {
        &self.autoload.classmap
    }

    /// Get all files entries.
    pub fn files_entries(&self) -> &[String] {
        &self.autoload.files
    }

    /// Get all PSR-4 paths as a flat list of (namespace, path) pairs.
    pub fn psr4_paths(&self) -> Vec<(&str, &str)> {
        let mut result = Vec::new();
        for (ns, val) in &self.autoload.psr4 {
            for path in val.as_vec() {
                result.push((ns.as_str(), path));
            }
        }
        result
    }

    /// Get all PSR-0 paths as a flat list of (namespace, path) pairs.
    pub fn psr0_paths(&self) -> Vec<(&str, &str)> {
        let mut result = Vec::new();
        for (ns, val) in &self.autoload.psr0 {
            for path in val.as_vec() {
                result.push((ns.as_str(), path));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_lock_entry() {
        use super::super::composer_json::ComposerLockPackage;
        let entry = ComposerLockPackage {
            name: "vendor/package".to_string(),
            version: Some("1.2.3".to_string()),
            autoload: Some(AutoloadConfig {
                psr4: {
                    let mut m = HashMap::new();
                    m.insert(
                        "Vendor\\Package\\".to_string(),
                        Psr4Value::Single("src/".to_string()),
                    );
                    m
                },
                ..Default::default()
            }),
            require: Some({
                let mut m = HashMap::new();
                m.insert("php".to_string(), ">=7.4".to_string());
                m.insert("other/dep".to_string(), "^2.0".to_string());
                m
            }),
            package_type: Some("library".to_string()),
            license: None,
        };

        let vendor_dir = PathBuf::from("/tmp/vendor");
        let pkg = ComposerPackage::from_lock_entry(&entry, &vendor_dir, None);
        assert_eq!(pkg.name(), "vendor/package");
        assert_eq!(pkg.version(), Some("1.2.3"));
        assert_eq!(pkg.relative_path(), Some("vendor/package/".to_string()));
        assert_eq!(pkg.requires_names().len(), 1);
        assert!(pkg.requires_names().contains(&"other/dep".to_string()));
        assert_eq!(pkg.license(), "proprietary?");
        assert!(pkg.is_copy());
        assert!(!pkg.did_copy());
        assert!(!pkg.is_delete());
        assert!(!pkg.did_delete());
    }

    #[test]
    fn test_psr4_namespaces() {
        let autoload = AutoloadConfig {
            psr4: {
                let mut m = HashMap::new();
                m.insert(
                    "Vendor\\Pkg\\".to_string(),
                    Psr4Value::Single("src/".to_string()),
                );
                m.insert(
                    "Vendor\\Lib\\".to_string(),
                    Psr4Value::Multiple(vec!["lib/".to_string(), "extra/".to_string()]),
                );
                m
            },
            ..Default::default()
        };
        let pkg = ComposerPackage::from_json_value("v/p", autoload, vec![], "MIT");
        let namespaces = pkg.psr4_namespaces();
        assert_eq!(namespaces.len(), 2);
        assert!(namespaces.contains(&"Vendor\\Pkg\\"));
        assert!(namespaces.contains(&"Vendor\\Lib\\"));
    }

    #[test]
    fn test_copy_delete_flags() {
        let pkg_auto = AutoloadConfig::default();
        let mut pkg = ComposerPackage::from_json_value("v/p", pkg_auto, vec![], "MIT");
        assert!(pkg.is_copy());
        assert!(!pkg.did_copy());
        assert!(!pkg.is_delete());
        assert!(!pkg.did_delete());

        pkg.set_copy(false);
        assert!(!pkg.is_copy());

        pkg.set_did_copy(true);
        assert!(pkg.did_copy());

        pkg.set_delete(true);
        assert!(pkg.is_delete());

        pkg.set_did_delete(true);
        assert!(pkg.did_delete());
    }
}
