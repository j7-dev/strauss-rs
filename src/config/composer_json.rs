use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

/// Represents the `autoload` (and `autoload-dev`) section of composer.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AutoloadConfig {
    /// PSR-4 autoload mappings: namespace prefix -> path(s)
    #[serde(default, rename = "psr-4")]
    pub psr4: HashMap<String, Psr4Value>,

    /// PSR-0 autoload mappings: namespace prefix -> path(s)
    #[serde(default, rename = "psr-0")]
    pub psr0: HashMap<String, Psr4Value>,

    /// Classmap directories/files
    #[serde(default)]
    pub classmap: Vec<String>,

    /// Files to always load
    #[serde(default)]
    pub files: Vec<String>,

    /// Directories/patterns to exclude from classmap generation
    #[serde(default, rename = "exclude-from-classmap")]
    pub exclude_from_classmap: Vec<String>,
}

impl AutoloadConfig {
    /// Get the first PSR-4 namespace key, if any.
    pub fn first_psr4_key(&self) -> Option<&str> {
        self.psr4.keys().next().map(|s| s.as_str())
    }

    /// Get the first PSR-0 namespace key, if any.
    pub fn first_psr0_key(&self) -> Option<&str> {
        self.psr0.keys().next().map(|s| s.as_str())
    }

    /// Flatten all autoload paths into a single Vec<String>.
    pub fn flat_paths(&self) -> Vec<String> {
        let mut result = Vec::new();
        for val in self.psr4.values() {
            match val {
                Psr4Value::Single(s) => result.push(s.clone()),
                Psr4Value::Multiple(v) => result.extend(v.iter().cloned()),
            }
        }
        for val in self.psr0.values() {
            match val {
                Psr4Value::Single(s) => result.push(s.clone()),
                Psr4Value::Multiple(v) => result.extend(v.iter().cloned()),
            }
        }
        result.extend(self.classmap.iter().cloned());
        result.extend(self.files.iter().cloned());
        result
    }

    /// Check if a given path (trimmed of slashes) matches any autoload path.
    pub fn contains_path(&self, target: &str) -> bool {
        let target_trimmed = target.trim_matches(|c| c == '/' || c == '\\');
        let target_with_slash = format!("{}/", target_trimmed);
        for path in self.flat_paths() {
            let path_trimmed = path.trim_matches(|c| c == '/' || c == '\\');
            let path_with_slash = format!("{}/", path_trimmed);
            if path_with_slash == target_with_slash {
                return true;
            }
        }
        false
    }
}

/// PSR-4/PSR-0 values can be a single string or an array of strings.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Psr4Value {
    Single(String),
    Multiple(Vec<String>),
}

impl Psr4Value {
    /// Get all paths as a Vec.
    pub fn as_vec(&self) -> Vec<&str> {
        match self {
            Psr4Value::Single(s) => vec![s.as_str()],
            Psr4Value::Multiple(v) => v.iter().map(|s| s.as_str()).collect(),
        }
    }
}

/// The `extra.strauss` configuration block in composer.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct StraussExtra {
    #[serde(default)]
    pub target_directory: Option<String>,

    #[serde(default)]
    pub namespace_prefix: Option<String>,

    #[serde(default)]
    pub classmap_prefix: Option<String>,

    /// Can be a string, bool, or null in the JSON. We capture as raw Value and interpret later.
    #[serde(default)]
    pub functions_prefix: Option<Value>,

    #[serde(default)]
    pub constants_prefix: Option<String>,

    /// Can be bool `true`/`false` or an array of paths.
    #[serde(default)]
    pub update_call_sites: Option<Value>,

    #[serde(default)]
    pub packages: Option<Vec<String>>,

    #[serde(default)]
    pub exclude_from_copy: Option<ExcludeConfigRaw>,

    #[serde(default)]
    pub exclude_from_prefix: Option<ExcludeConfigRaw>,

    #[serde(default)]
    pub override_autoload: Option<HashMap<String, Value>>,

    #[serde(default)]
    pub delete_vendor_files: Option<bool>,

    #[serde(default)]
    pub delete_vendor_packages: Option<bool>,

    #[serde(default)]
    pub classmap_output: Option<bool>,

    #[serde(default)]
    pub namespace_replacement_patterns: Option<HashMap<String, String>>,

    #[serde(default)]
    pub include_modified_date: Option<bool>,

    #[serde(default)]
    pub include_author: Option<bool>,

    #[serde(default)]
    pub include_root_autoload: Option<bool>,

    // Mozart backward-compatible keys
    #[serde(default)]
    pub vendor_directory: Option<String>,
}

/// The `extra.mozart` configuration block in composer.json (backward compatibility).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct MozartConfig {
    /// Maps to `target_directory`
    #[serde(default)]
    pub dep_directory: Option<String>,

    /// Maps to `namespace_prefix`
    #[serde(default)]
    pub dep_namespace: Option<String>,

    /// Maps to `classmap_prefix`
    #[serde(default)]
    pub classmap_prefix: Option<String>,

    /// Maps to `exclude_from_prefix.packages`
    #[serde(default)]
    pub exclude_packages: Option<Vec<String>>,

    /// Maps to `functions_prefix`
    #[serde(default)]
    pub function_prefix: Option<Value>,

    /// Maps to `packages`
    #[serde(default)]
    pub packages: Option<Vec<String>>,

    /// Maps to `delete_vendor_files`
    #[serde(default)]
    pub delete_vendor_files: Option<bool>,

    /// Maps to `delete_vendor_packages`
    #[serde(default)]
    pub delete_vendor_packages: Option<bool>,

    #[serde(default)]
    pub override_autoload: Option<HashMap<String, Value>>,
}

/// Raw exclude config from JSON (before merging defaults).
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExcludeConfigRaw {
    #[serde(default)]
    pub packages: Option<Vec<String>>,

    #[serde(default)]
    pub namespaces: Option<Vec<String>>,

    #[serde(default)]
    pub file_patterns: Option<Vec<String>>,
}

/// Represents the `extra` section of composer.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposerExtra {
    #[serde(default)]
    pub strauss: Option<StraussExtra>,

    #[serde(default)]
    pub mozart: Option<MozartConfig>,
}

/// Represents the `config` section of composer.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposerConfigSection {
    #[serde(default, rename = "vendor-dir")]
    pub vendor_dir: Option<String>,
}

/// Represents a full `composer.json` file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposerJson {
    #[serde(default)]
    pub name: Option<String>,

    #[serde(default)]
    pub autoload: Option<AutoloadConfig>,

    #[serde(default, rename = "autoload-dev")]
    pub autoload_dev: Option<AutoloadConfig>,

    #[serde(default)]
    pub require: Option<HashMap<String, String>>,

    #[serde(default, rename = "require-dev")]
    pub require_dev: Option<HashMap<String, String>>,

    #[serde(default)]
    pub extra: Option<ComposerExtra>,

    #[serde(default)]
    pub license: Option<LicenseValue>,

    #[serde(default)]
    pub config: Option<ComposerConfigSection>,

    #[serde(default)]
    pub authors: Option<Vec<AuthorEntry>>,

    /// Catch-all for any other fields
    #[serde(default, rename = "type")]
    pub package_type: Option<String>,

    #[serde(default)]
    pub version: Option<String>,
}

impl ComposerJson {
    /// Parse a composer.json from a file path.
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let parsed: ComposerJson = serde_json::from_str(&content)?;
        Ok(parsed)
    }

    /// Get the vendor directory (from config.vendor-dir), defaulting to "vendor".
    pub fn vendor_dir(&self) -> &str {
        self.config
            .as_ref()
            .and_then(|c| c.vendor_dir.as_deref())
            .unwrap_or("vendor")
    }

    /// Get the effective autoload config (empty default if missing).
    pub fn autoload(&self) -> AutoloadConfig {
        self.autoload.clone().unwrap_or_default()
    }

    /// Get the effective autoload-dev config (empty default if missing).
    pub fn autoload_dev(&self) -> AutoloadConfig {
        self.autoload_dev.clone().unwrap_or_default()
    }

    /// Get the first author name, or derive from package name.
    pub fn author_name(&self) -> String {
        if let Some(authors) = &self.authors {
            if let Some(first) = authors.first() {
                if let Some(name) = &first.name {
                    return name.clone();
                }
            }
        }
        // Fall back to vendor part of package name
        if let Some(name) = &self.name {
            if let Some(vendor) = name.split('/').next() {
                return vendor.to_string();
            }
        }
        String::from("unknown")
    }

    /// Get all require package names (excluding php and ext-*).
    pub fn require_names(&self) -> Vec<String> {
        self.require
            .as_ref()
            .map(|r| {
                r.keys()
                    .filter(|k| *k != "php" && !k.starts_with("ext-"))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// License can be a single string or an array of strings.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LicenseValue {
    Single(String),
    Multiple(Vec<String>),
}

impl LicenseValue {
    /// Get the license as a comma-separated string.
    pub fn as_string(&self) -> String {
        match self {
            LicenseValue::Single(s) => s.clone(),
            LicenseValue::Multiple(v) => v.join(","),
        }
    }
}

impl Default for LicenseValue {
    fn default() -> Self {
        LicenseValue::Single(String::from("proprietary?"))
    }
}

/// An author entry in composer.json.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AuthorEntry {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub homepage: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
}

/// Represents the `composer.lock` file.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposerLock {
    #[serde(default)]
    pub packages: Vec<ComposerLockPackage>,

    #[serde(default, rename = "packages-dev")]
    pub packages_dev: Vec<ComposerLockPackage>,
}

impl ComposerLock {
    /// Parse a composer.lock from a file path.
    pub fn from_path(path: &std::path::Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let parsed: ComposerLock = serde_json::from_str(&content)?;
        Ok(parsed)
    }

    /// Find a package by name in either packages or packages-dev.
    pub fn find_package(&self, name: &str) -> Option<&ComposerLockPackage> {
        self.packages
            .iter()
            .chain(self.packages_dev.iter())
            .find(|p| p.name == name)
    }
}

/// A single package entry in composer.lock.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ComposerLockPackage {
    #[serde(default)]
    pub name: String,

    #[serde(default)]
    pub version: Option<String>,

    #[serde(default)]
    pub autoload: Option<AutoloadConfig>,

    #[serde(default)]
    pub require: Option<HashMap<String, String>>,

    #[serde(default, rename = "type")]
    pub package_type: Option<String>,

    #[serde(default)]
    pub license: Option<LicenseValue>,
}

impl ComposerLockPackage {
    /// Get the effective autoload config (empty default if missing).
    pub fn autoload(&self) -> AutoloadConfig {
        self.autoload.clone().unwrap_or_default()
    }

    /// Get require package names (excluding php and ext-*).
    pub fn require_names(&self) -> Vec<String> {
        self.require
            .as_ref()
            .map(|r| {
                r.keys()
                    .filter(|k| *k != "php" && !k.starts_with("ext-"))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the license string.
    pub fn license_string(&self) -> String {
        self.license
            .as_ref()
            .map(|l| l.as_string())
            .unwrap_or_else(|| String::from("proprietary?"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_composer_json() {
        let json = r#"{
            "name": "vendor/project",
            "require": {
                "php": ">=7.4",
                "some/package": "^1.0"
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("vendor/project"));
        let names = parsed.require_names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&String::from("some/package")));
    }

    #[test]
    fn test_parse_autoload_psr4() {
        let json = r#"{
            "autoload": {
                "psr-4": {
                    "Vendor\\Package\\": "src/"
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let autoload = parsed.autoload();
        assert_eq!(autoload.first_psr4_key(), Some("Vendor\\Package\\"));
    }

    #[test]
    fn test_parse_autoload_psr4_multiple_paths() {
        let json = r#"{
            "autoload": {
                "psr-4": {
                    "Vendor\\Package\\": ["src/", "lib/"]
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let autoload = parsed.autoload();
        let key = autoload.first_psr4_key().unwrap();
        let val = autoload.psr4.get(key).unwrap();
        assert_eq!(val.as_vec().len(), 2);
    }

    #[test]
    fn test_parse_strauss_extra() {
        let json = r#"{
            "name": "vendor/project",
            "extra": {
                "strauss": {
                    "target_directory": "vendor-prefixed",
                    "namespace_prefix": "MyPrefix\\",
                    "classmap_prefix": "MyPrefix_",
                    "delete_vendor_packages": true,
                    "packages": ["some/package"]
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap();
        let strauss = extra.strauss.unwrap();
        assert_eq!(
            strauss.target_directory.as_deref(),
            Some("vendor-prefixed")
        );
        assert_eq!(
            strauss.namespace_prefix.as_deref(),
            Some("MyPrefix\\")
        );
        assert_eq!(strauss.classmap_prefix.as_deref(), Some("MyPrefix_"));
        assert_eq!(strauss.delete_vendor_packages, Some(true));
        assert_eq!(
            strauss.packages.as_ref().unwrap(),
            &vec!["some/package".to_string()]
        );
    }

    #[test]
    fn test_parse_mozart_extra() {
        let json = r#"{
            "name": "vendor/project",
            "extra": {
                "mozart": {
                    "dep_directory": "dep_target",
                    "dep_namespace": "Mozart\\Prefix\\",
                    "classmap_prefix": "Mozart_Prefix_",
                    "exclude_packages": ["excluded/pkg"],
                    "delete_vendor_files": true
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap();
        let mozart = extra.mozart.unwrap();
        assert_eq!(mozart.dep_directory.as_deref(), Some("dep_target"));
        assert_eq!(
            mozart.dep_namespace.as_deref(),
            Some("Mozart\\Prefix\\")
        );
        assert_eq!(mozart.classmap_prefix.as_deref(), Some("Mozart_Prefix_"));
        assert_eq!(
            mozart.exclude_packages.as_ref().unwrap(),
            &vec!["excluded/pkg".to_string()]
        );
        assert_eq!(mozart.delete_vendor_files, Some(true));
    }

    #[test]
    fn test_parse_composer_lock() {
        let json = r#"{
            "packages": [
                {
                    "name": "some/package",
                    "version": "1.0.0",
                    "autoload": {
                        "psr-4": {
                            "Some\\Package\\": "src/"
                        }
                    },
                    "require": {
                        "php": ">=7.4",
                        "other/dep": "^2.0"
                    },
                    "type": "library",
                    "license": ["MIT"]
                }
            ],
            "packages-dev": []
        }"#;
        let parsed: ComposerLock = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.packages.len(), 1);
        let pkg = &parsed.packages[0];
        assert_eq!(pkg.name, "some/package");
        assert_eq!(pkg.version.as_deref(), Some("1.0.0"));
        let req_names = pkg.require_names();
        assert_eq!(req_names.len(), 1);
        assert!(req_names.contains(&String::from("other/dep")));
        assert_eq!(pkg.license_string(), "MIT");
    }

    #[test]
    fn test_license_single() {
        let json = r#"{"license": "MIT"}"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.license.unwrap().as_string(), "MIT");
    }

    #[test]
    fn test_license_array() {
        let json = r#"{"license": ["MIT", "Apache-2.0"]}"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.license.unwrap().as_string(), "MIT,Apache-2.0");
    }

    #[test]
    fn test_vendor_dir_default() {
        let json = r#"{}"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.vendor_dir(), "vendor");
    }

    #[test]
    fn test_vendor_dir_custom() {
        let json = r#"{"config": {"vendor-dir": "libs"}}"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.vendor_dir(), "libs");
    }

    #[test]
    fn test_update_call_sites_bool_true() {
        let json = r#"{
            "extra": {
                "strauss": {
                    "update_call_sites": true
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap().strauss.unwrap();
        let val = extra.update_call_sites.unwrap();
        assert_eq!(val.as_bool(), Some(true));
    }

    #[test]
    fn test_update_call_sites_array() {
        let json = r#"{
            "extra": {
                "strauss": {
                    "update_call_sites": ["src/", "includes/"]
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap().strauss.unwrap();
        let val = extra.update_call_sites.unwrap();
        assert!(val.is_array());
        let arr = val.as_array().unwrap();
        assert_eq!(arr.len(), 2);
    }

    #[test]
    fn test_functions_prefix_string() {
        let json = r#"{
            "extra": {
                "strauss": {
                    "functions_prefix": "my_prefix_"
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap().strauss.unwrap();
        let val = extra.functions_prefix.unwrap();
        assert_eq!(val.as_str(), Some("my_prefix_"));
    }

    #[test]
    fn test_functions_prefix_false() {
        let json = r#"{
            "extra": {
                "strauss": {
                    "functions_prefix": false
                }
            }
        }"#;
        let parsed: ComposerJson = serde_json::from_str(json).unwrap();
        let extra = parsed.extra.unwrap().strauss.unwrap();
        let val = extra.functions_prefix.unwrap();
        assert_eq!(val.as_bool(), Some(false));
    }
}
