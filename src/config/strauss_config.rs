use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde_json::Value;

use super::composer_json::{AutoloadConfig, ComposerJson};

/// Configuration for which packages/namespaces/file patterns to exclude.
#[derive(Debug, Clone, Default)]
pub struct ExcludeConfig {
    pub packages: Vec<String>,
    pub namespaces: Vec<String>,
    pub file_patterns: Vec<String>,
}

/// The fully-resolved Strauss configuration, built from composer.json
/// (with extra.strauss and/or extra.mozart), plus CLI overrides.
///
/// This mirrors every field from the PHP `StraussConfig` class.
#[derive(Debug, Clone)]
pub struct StraussConfig {
    /// The directory containing `composer.json`. Always ends with `/`.
    pub project_directory: String,

    /// The output directory name (relative to project_directory). Default: "vendor-prefixed".
    pub target_directory: String,

    /// The vendor directory name (relative to project_directory). Default: "vendor".
    pub vendor_directory: String,

    /// Prefix for PHP namespaces.
    pub namespace_prefix: Option<String>,

    /// Prefix for classmap entries.
    pub classmap_prefix: Option<String>,

    /// Prefix for PHP functions. None = disabled.
    pub functions_prefix: Option<String>,

    /// Prefix for PHP constants.
    pub constants_prefix: Option<String>,

    /// Paths to update call sites in.
    pub update_call_sites: Option<Vec<String>>,

    /// Packages to copy and prefix. If empty at config time, derived from require keys.
    pub packages: Vec<String>,

    /// Exclude rules for the copy step.
    pub exclude_from_copy: ExcludeConfig,

    /// Exclude rules for the prefix step.
    pub exclude_from_prefix: ExcludeConfig,

    /// Per-package autoload overrides. Keys are package names, values are raw autoload JSON.
    pub override_autoload: HashMap<String, Value>,

    /// Delete individual vendor files after prefixing?
    pub delete_vendor_files: bool,

    /// Delete entire vendor packages after prefixing?
    pub delete_vendor_packages: bool,

    /// Whether to generate a classmap output.
    pub classmap_output: Option<bool>,

    /// Regex capture => replacement patterns for namespace rewriting.
    pub namespace_replacement_patterns: HashMap<String, String>,

    /// Include a "Modified date" comment in the header of changed files?
    pub include_modified_date: bool,

    /// Include the author name in the header of changed files?
    pub include_author: bool,

    /// Dry run mode: print changes instead of writing files.
    pub dry_run: bool,

    /// Include the root project's autoload in the generated Strauss autoloader?
    pub include_root_autoload: bool,

    /// Internal: the functions prefix state for tracking not-set vs disabled
    functions_prefix_state: FunctionsPrefixState,
}

/// Internal state for the functions_prefix field, to distinguish "not set" from "explicitly disabled".
#[derive(Debug, Clone)]
enum FunctionsPrefixState {
    /// Not configured at all; will derive from classmap_prefix at access time.
    NotSet,
    /// Explicitly disabled (PHP `false` or empty string).
    Disabled,
    /// An explicit prefix string.
    Prefix(String),
}

impl StraussConfig {
    /// Build a StraussConfig from a `composer.json` file at the given path.
    ///
    /// This reads `extra.mozart` first (for backward compatibility), then
    /// overlays `extra.strauss` on top, then applies default inference logic
    /// exactly matching the PHP constructor.
    pub fn from_composer_json(composer_json_path: &Path) -> Result<Self> {
        let composer_json = ComposerJson::from_path(composer_json_path)
            .with_context(|| format!("Failed to read {}", composer_json_path.display()))?;

        let project_directory = composer_json_path
            .parent()
            .unwrap_or_else(|| Path::new("."));
        let project_dir_str = normalize_dir_path(project_directory);

        let autoload = composer_json.autoload();
        let package_name = composer_json.name.as_deref().unwrap_or("__root__");

        // Start with defaults
        let mut config = StraussConfig {
            project_directory: project_dir_str,
            target_directory: String::from("vendor-prefixed"),
            vendor_directory: String::from(composer_json.vendor_dir()),
            namespace_prefix: None,
            classmap_prefix: None,
            functions_prefix: None,
            functions_prefix_state: FunctionsPrefixState::NotSet,
            constants_prefix: None,
            update_call_sites: Some(Vec::new()), // default: disabled (empty array)
            packages: Vec::new(),
            exclude_from_copy: ExcludeConfig::default(),
            exclude_from_prefix: ExcludeConfig::default(),
            override_autoload: HashMap::new(),
            delete_vendor_files: false,
            delete_vendor_packages: false,
            classmap_output: None,
            namespace_replacement_patterns: HashMap::new(),
            include_modified_date: true,
            include_author: true,
            dry_run: false,
            include_root_autoload: false,
        };

        // --- Phase 1: Apply Mozart config (backward compat) ---
        if let Some(extra) = &composer_json.extra {
            if let Some(mozart) = &extra.mozart {
                // Mozart default: delete vendor files
                config.delete_vendor_files = true;

                if let Some(ref dir) = mozart.dep_directory {
                    config.target_directory = dir.clone();
                }
                if let Some(ref ns) = mozart.dep_namespace {
                    config.namespace_prefix = Some(ns.clone());
                }
                if let Some(ref cp) = mozart.classmap_prefix {
                    config.classmap_prefix = Some(cp.clone());
                }
                if let Some(ref pkgs) = mozart.exclude_packages {
                    config.exclude_from_prefix.packages = pkgs.clone();
                }
                if let Some(ref fp) = mozart.function_prefix {
                    config.functions_prefix_state = parse_functions_prefix(fp);
                    config.functions_prefix = match &config.functions_prefix_state {
                        FunctionsPrefixState::Prefix(s) => Some(s.clone()),
                        _ => None,
                    };
                }
                if let Some(ref pkgs) = mozart.packages {
                    config.packages = pkgs.clone();
                }
                if let Some(dvf) = mozart.delete_vendor_files {
                    config.delete_vendor_files = dvf;
                }
                if let Some(dvp) = mozart.delete_vendor_packages {
                    config.delete_vendor_packages = dvp;
                }
                if let Some(ref oa) = mozart.override_autoload {
                    config.override_autoload = oa.clone();
                }
            }
        }

        // --- Phase 2: Apply Strauss config (overrides Mozart) ---
        if let Some(extra) = &composer_json.extra {
            if let Some(strauss) = &extra.strauss {
                if let Some(ref dir) = strauss.target_directory {
                    config.target_directory = dir.clone();
                }
                if let Some(ref ns) = strauss.namespace_prefix {
                    config.namespace_prefix = Some(ns.clone());
                }
                if let Some(ref cp) = strauss.classmap_prefix {
                    config.classmap_prefix = Some(cp.clone());
                }
                if let Some(ref fp) = strauss.functions_prefix {
                    config.functions_prefix_state = parse_functions_prefix(fp);
                    config.functions_prefix = match &config.functions_prefix_state {
                        FunctionsPrefixState::Prefix(s) => Some(s.clone()),
                        _ => None,
                    };
                }
                if let Some(ref cp) = strauss.constants_prefix {
                    config.constants_prefix = Some(cp.clone());
                }
                if let Some(ref pkgs) = strauss.packages {
                    config.packages = pkgs.clone();
                }
                if let Some(ref vd) = strauss.vendor_directory {
                    config.vendor_directory = vd.clone();
                }
                if let Some(dvf) = strauss.delete_vendor_files {
                    config.delete_vendor_files = dvf;
                }
                if let Some(dvp) = strauss.delete_vendor_packages {
                    config.delete_vendor_packages = dvp;
                }
                if let Some(co) = strauss.classmap_output {
                    config.classmap_output = Some(co);
                }
                if let Some(imd) = strauss.include_modified_date {
                    config.include_modified_date = imd;
                }
                if let Some(ia) = strauss.include_author {
                    config.include_author = ia;
                }
                if let Some(ira) = strauss.include_root_autoload {
                    config.include_root_autoload = ira;
                }
                if let Some(ref nrp) = strauss.namespace_replacement_patterns {
                    config.namespace_replacement_patterns = nrp.clone();
                }
                if let Some(ref oa) = strauss.override_autoload {
                    config.override_autoload = oa.clone();
                }

                // exclude_from_copy
                if let Some(ref efc) = strauss.exclude_from_copy {
                    if let Some(ref pkgs) = efc.packages {
                        config.exclude_from_copy.packages = pkgs.clone();
                    }
                    if let Some(ref nss) = efc.namespaces {
                        config.exclude_from_copy.namespaces = nss.clone();
                    }
                    if let Some(ref fps) = efc.file_patterns {
                        config.exclude_from_copy.file_patterns = fps.clone();
                    }
                }

                // exclude_from_prefix
                if let Some(ref efp) = strauss.exclude_from_prefix {
                    if let Some(ref pkgs) = efp.packages {
                        config.exclude_from_prefix.packages = pkgs.clone();
                    }
                    if let Some(ref nss) = efp.namespaces {
                        config.exclude_from_prefix.namespaces = nss.clone();
                    }
                    if let Some(ref fps) = efp.file_patterns {
                        config.exclude_from_prefix.file_patterns = fps.clone();
                    }
                }

                // update_call_sites: can be true, false, or array of strings
                if let Some(ref ucs) = strauss.update_call_sites {
                    if let Some(true) = ucs.as_bool() {
                        config.update_call_sites = None; // use autoload key
                    } else if let Some(false) = ucs.as_bool() {
                        config.update_call_sites = Some(Vec::new()); // disabled
                    } else if let Some(arr) = ucs.as_array() {
                        let paths: Vec<String> = arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect();
                        config.update_call_sites = Some(paths);
                    }
                }
            }
        }

        // --- Phase 3: Infer defaults ---

        // Infer namespace_prefix
        if config.namespace_prefix.is_none() {
            if let Some(key) = autoload.first_psr4_key() {
                config.namespace_prefix = Some(key.to_string());
            } else if let Some(key) = autoload.first_psr0_key() {
                config.namespace_prefix = Some(key.to_string());
            } else if package_name != "__root__" {
                config.namespace_prefix = Some(derive_namespace_from_package_name(package_name));
            } else if let Some(ref cp) = config.classmap_prefix {
                let ns = cp.trim_end_matches('_').to_string();
                config.namespace_prefix = Some(ns);
            }
        }

        // Infer classmap_prefix
        if config.classmap_prefix.is_none() {
            if let Some(key) = autoload.first_psr4_key() {
                config.classmap_prefix = Some(key.replace('\\', "_"));
            } else if let Some(key) = autoload.first_psr0_key() {
                config.classmap_prefix = Some(key.replace('\\', "_"));
            } else if package_name != "__root__" {
                config.classmap_prefix =
                    Some(derive_classmap_prefix_from_package_name(package_name));
            } else if let Some(ref ns) = config.namespace_prefix {
                let mut cp = ns.replace(|c: char| !c.is_alphanumeric() && c != '/' && c != '_', "_");
                cp = cp.trim_end_matches('_').to_string();
                cp.push('_');
                config.classmap_prefix = Some(cp);
            }
        }

        // Infer packages from require keys
        if config.packages.is_empty() {
            config.packages = composer_json.require_names();
        }

        // Infer classmap_output (auto-detect)
        if config.classmap_output.is_none() {
            let target_trimmed = format!(
                "{}/",
                config.target_directory.trim_matches(|c| c == '/' || c == '\\')
            );
            let is_in_autoload = autoload.contains_path(&target_trimmed);
            config.classmap_output = Some(!is_in_autoload);
        }

        Ok(config)
    }

    // ============================================================
    // Getters
    // ============================================================

    /// The project directory, always ending with `/`.
    /// In dry-run mode, prefixed with `mem:/`.
    pub fn project_directory(&self) -> String {
        if self.dry_run {
            format!("mem:/{}", &self.project_directory)
        } else {
            self.project_directory.clone()
        }
    }

    /// The raw project directory (without dry-run prefix).
    pub fn project_directory_raw(&self) -> &str {
        &self.project_directory
    }

    /// The full target directory path (project_directory + target_directory), with trailing `/`.
    pub fn target_directory(&self) -> String {
        let trimmed = self.target_directory.trim_matches(|c| c == '/' || c == '\\');
        format!("{}{}/", self.project_directory(), trimmed)
    }

    /// The target directory name only (without project prefix), trimmed.
    pub fn target_directory_name(&self) -> &str {
        self.target_directory
            .trim_matches(|c| c == '/' || c == '\\')
    }

    /// The full vendor directory path (project_directory + vendor_directory), with trailing `/`.
    pub fn vendor_directory(&self) -> String {
        let trimmed = self.vendor_directory.trim_matches(|c| c == '/' || c == '\\');
        format!("{}{}/", self.project_directory(), trimmed)
    }

    /// The vendor directory name only (without project prefix), trimmed.
    pub fn vendor_directory_name(&self) -> &str {
        self.vendor_directory
            .trim_matches(|c| c == '/' || c == '\\')
    }

    /// The namespace prefix, without leading or trailing backslashes.
    /// Returns None if no prefix is configured.
    pub fn namespace_prefix(&self) -> Option<&str> {
        self.namespace_prefix
            .as_deref()
            .map(|s| s.trim_matches('\\'))
    }

    /// The classmap prefix. Returns None if not configured.
    pub fn classmap_prefix(&self) -> Option<&str> {
        self.classmap_prefix.as_deref()
    }

    /// The functions prefix.
    /// - If not set, derives from classmap_prefix (lowercased).
    /// - If explicitly disabled, returns None.
    /// - Otherwise returns the configured string.
    pub fn get_functions_prefix(&self) -> Option<String> {
        match &self.functions_prefix_state {
            FunctionsPrefixState::NotSet => {
                self.classmap_prefix.as_ref().map(|cp| cp.to_lowercase())
            }
            FunctionsPrefixState::Disabled => None,
            FunctionsPrefixState::Prefix(s) => {
                if s.is_empty() {
                    None
                } else {
                    Some(s.clone())
                }
            }
        }
    }

    /// The constants prefix. Returns None if not configured.
    pub fn constants_prefix(&self) -> Option<&str> {
        self.constants_prefix.as_deref()
    }

    /// Update call sites configuration.
    /// - None => use autoload key
    /// - Some(empty) => disabled
    /// - Some(paths) => update these paths
    pub fn update_call_sites(&self) -> Option<&[String]> {
        self.update_call_sites.as_deref()
    }

    /// The list of packages to process.
    pub fn packages(&self) -> &[String] {
        &self.packages
    }

    /// Exclude config for the copy step.
    pub fn exclude_from_copy(&self) -> &ExcludeConfig {
        &self.exclude_from_copy
    }

    /// Packages excluded from copying.
    pub fn exclude_packages_from_copy(&self) -> &[String] {
        &self.exclude_from_copy.packages
    }

    /// Namespaces excluded from copying.
    pub fn exclude_namespaces_from_copy(&self) -> &[String] {
        &self.exclude_from_copy.namespaces
    }

    /// File patterns excluded from copying.
    pub fn exclude_file_patterns_from_copy(&self) -> &[String] {
        &self.exclude_from_copy.file_patterns
    }

    /// Exclude config for the prefix step.
    pub fn exclude_from_prefix(&self) -> &ExcludeConfig {
        &self.exclude_from_prefix
    }

    /// Packages excluded from prefixing.
    pub fn exclude_packages_from_prefixing(&self) -> &[String] {
        &self.exclude_from_prefix.packages
    }

    /// Namespaces excluded from prefixing, trimmed of leading/trailing slashes.
    pub fn exclude_namespaces_from_prefixing(&self) -> Vec<String> {
        self.exclude_from_prefix
            .namespaces
            .iter()
            .map(|ns| ns.trim_matches(|c| c == '/' || c == '\\').to_string())
            .collect()
    }

    /// File patterns excluded from prefixing.
    pub fn exclude_file_patterns_from_prefixing(&self) -> &[String] {
        &self.exclude_from_prefix.file_patterns
    }

    /// Per-package autoload overrides.
    pub fn override_autoload(&self) -> &HashMap<String, Value> {
        &self.override_autoload
    }

    /// Whether to delete vendor files after prefixing.
    pub fn delete_vendor_files(&self) -> bool {
        self.delete_vendor_files
    }

    /// Whether to delete vendor packages after prefixing.
    pub fn delete_vendor_packages(&self) -> bool {
        self.delete_vendor_packages
    }

    /// Whether to generate a classmap output file.
    /// The Option is resolved at construction time, so this always returns a concrete bool.
    pub fn classmap_output(&self) -> bool {
        self.classmap_output.unwrap_or(true)
    }

    /// Raw classmap_output option (None = was auto-detected).
    pub fn classmap_output_option(&self) -> Option<bool> {
        self.classmap_output
    }

    /// Namespace replacement patterns (regex capture => replacement).
    pub fn namespace_replacement_patterns(&self) -> &HashMap<String, String> {
        &self.namespace_replacement_patterns
    }

    /// Whether to include the modified date in file headers.
    pub fn include_modified_date(&self) -> bool {
        self.include_modified_date
    }

    /// Whether to include the author in file headers.
    pub fn include_author(&self) -> bool {
        self.include_author
    }

    /// Whether dry-run mode is active.
    pub fn dry_run(&self) -> bool {
        self.dry_run
    }

    /// Whether to include the root autoload in the Strauss autoloader.
    pub fn include_root_autoload(&self) -> bool {
        self.include_root_autoload
    }

    /// Whether to create class aliases (needed when deleting vendor packages/files
    /// or when the target directory is "vendor").
    pub fn create_aliases(&self) -> bool {
        self.delete_vendor_packages
            || self.delete_vendor_files
            || self.target_directory.trim_matches(|c| c == '/' || c == '\\') == "vendor"
    }

    // ============================================================
    // Setters (for CLI overrides and runtime mutation)
    // ============================================================

    /// Set the project directory.
    pub fn set_project_directory(&mut self, dir: String) {
        self.project_directory = normalize_dir_str(&dir);
    }

    /// Set the target directory (relative name).
    pub fn set_target_directory(&mut self, dir: String) {
        self.target_directory = dir;
    }

    /// Set the vendor directory (relative name).
    pub fn set_vendor_directory(&mut self, dir: String) {
        self.vendor_directory = dir;
    }

    /// Set the namespace prefix.
    pub fn set_namespace_prefix(&mut self, prefix: String) {
        self.namespace_prefix = Some(prefix);
    }

    /// Set the classmap prefix.
    pub fn set_classmap_prefix(&mut self, prefix: String) {
        self.classmap_prefix = Some(prefix);
    }

    /// Set the functions prefix. Pass None to disable.
    pub fn set_functions_prefix(&mut self, prefix: Option<String>) {
        match prefix {
            None => {
                self.functions_prefix_state = FunctionsPrefixState::Disabled;
                self.functions_prefix = None;
            }
            Some(s) if s.is_empty() => {
                self.functions_prefix_state = FunctionsPrefixState::Disabled;
                self.functions_prefix = None;
            }
            Some(s) => {
                self.functions_prefix_state = FunctionsPrefixState::Prefix(s.clone());
                self.functions_prefix = Some(s);
            }
        }
    }

    /// Set the constants prefix.
    pub fn set_constants_prefix(&mut self, prefix: Option<String>) {
        self.constants_prefix = prefix;
    }

    /// Set update_call_sites from a CLI string argument.
    ///
    /// - "false" => disabled (empty vec)
    /// - "true"  => use autoload key (None)
    /// - "src,includes" => specific paths
    pub fn set_update_call_sites_from_string(&mut self, value: &str) {
        match value {
            "false" => self.update_call_sites = Some(Vec::new()),
            "true" => self.update_call_sites = None,
            other => {
                let paths: Vec<String> = other
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                self.update_call_sites = Some(paths);
            }
        }
    }

    /// Set update_call_sites directly.
    pub fn set_update_call_sites(&mut self, value: Option<Vec<String>>) {
        self.update_call_sites = value;
    }

    /// Set the list of packages to process.
    pub fn set_packages(&mut self, packages: Vec<String>) {
        self.packages = packages;
    }

    /// Set the exclude-from-copy configuration.
    pub fn set_exclude_from_copy(&mut self, config: ExcludeConfig) {
        self.exclude_from_copy = config;
    }

    /// Set the exclude-from-prefix configuration.
    pub fn set_exclude_from_prefix(&mut self, config: ExcludeConfig) {
        self.exclude_from_prefix = config;
    }

    /// Set packages excluded from prefixing (backward compat with Mozart exclude_packages).
    pub fn set_exclude_packages_from_prefixing(&mut self, packages: Vec<String>) {
        self.exclude_from_prefix.packages = packages;
    }

    /// Set the override_autoload map.
    pub fn set_override_autoload(&mut self, overrides: HashMap<String, Value>) {
        self.override_autoload = overrides;
    }

    /// Set whether to delete vendor files.
    pub fn set_delete_vendor_files(&mut self, delete: bool) {
        self.delete_vendor_files = delete;
    }

    /// Set whether to delete vendor packages.
    pub fn set_delete_vendor_packages(&mut self, delete: bool) {
        self.delete_vendor_packages = delete;
    }

    /// Set the classmap_output flag.
    pub fn set_classmap_output(&mut self, output: bool) {
        self.classmap_output = Some(output);
    }

    /// Set namespace replacement patterns.
    pub fn set_namespace_replacement_patterns(&mut self, patterns: HashMap<String, String>) {
        self.namespace_replacement_patterns = patterns;
    }

    /// Set whether to include the modified date in file headers.
    pub fn set_include_modified_date(&mut self, include: bool) {
        self.include_modified_date = include;
    }

    /// Set whether to include the author in file headers.
    pub fn set_include_author(&mut self, include: bool) {
        self.include_author = include;
    }

    /// Set dry-run mode.
    pub fn set_dry_run(&mut self, dry_run: bool) {
        self.dry_run = dry_run;
    }

    /// Set whether to include root autoload.
    pub fn set_include_root_autoload(&mut self, include: bool) {
        self.include_root_autoload = include;
    }

    /// Get namespaces excluded from prefixing.
    pub fn get_exclude_namespaces_from_prefixing(&self) -> Vec<String> {
        self.exclude_from_prefix.namespaces.clone()
    }
}

// ============================================================
// Helper functions
// ============================================================

/// Parse a serde_json::Value representing functions_prefix into our internal state.
fn parse_functions_prefix(value: &Value) -> FunctionsPrefixState {
    match value {
        Value::Bool(false) | Value::Null => FunctionsPrefixState::Disabled,
        Value::Bool(true) => {
            // `true` means "use default" which is the same as not set
            FunctionsPrefixState::NotSet
        }
        Value::String(s) if s.is_empty() => FunctionsPrefixState::Disabled,
        Value::String(s) => FunctionsPrefixState::Prefix(s.clone()),
        _ => FunctionsPrefixState::Disabled,
    }
}

/// Derive a namespace prefix from a composer package name.
///
/// "vendor/package" => "Vendor\Package\"
///
/// Non-word characters (except `/`) are replaced with `_`, then each segment
/// after `_`, `\`, or start-of-string has its first letter uppercased.
fn derive_namespace_from_package_name(name: &str) -> String {
    // Replace non-word chars (except /) with underscore
    let replaced: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '/' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Replace / with backslash, add trailing backslash
    let with_ns = format!("{}\\", replaced.replace('/', "\\"));

    // Uppercase first letter after ^, _, or \
    let mut result = String::with_capacity(with_ns.len());
    let mut capitalize_next = true;
    for c in with_ns.chars() {
        if c == '_' || c == '\\' {
            result.push(c);
            capitalize_next = true;
        } else if capitalize_next {
            for uc in c.to_uppercase() {
                result.push(uc);
            }
            capitalize_next = false;
        } else {
            result.push(c);
        }
    }

    result
}

/// Derive a classmap prefix from a composer package name.
///
/// "vendor/package" => "Vendor_Package_"
///
/// Same logic as namespace but with `\` replaced by `_`.
fn derive_classmap_prefix_from_package_name(name: &str) -> String {
    let ns = derive_namespace_from_package_name(name);
    // The namespace derivation ends with `\`, replace all `\` with `_`
    ns.replace('\\', "_")
}

/// Normalize a Path to a string with forward slashes and a trailing slash.
fn normalize_dir_path(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    normalize_dir_str(&s)
}

/// Ensure a directory string uses forward slashes and ends with `/`.
fn normalize_dir_str(s: &str) -> String {
    let s = s.replace('\\', "/");
    if s.ends_with('/') {
        s
    } else {
        format!("{}/", s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper: write a composer.json to a temp dir and return the path.
    fn write_composer_json(dir: &TempDir, content: &str) -> PathBuf {
        let path = dir.path().join("composer.json");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_derive_namespace_from_package_name() {
        assert_eq!(
            derive_namespace_from_package_name("vendor/package"),
            "Vendor\\Package\\"
        );
        assert_eq!(
            derive_namespace_from_package_name("brian-henry-ie/my-plugin"),
            "Brian_Henry_Ie\\My_Plugin\\"
        );
    }

    #[test]
    fn test_derive_classmap_prefix_from_package_name() {
        assert_eq!(
            derive_classmap_prefix_from_package_name("vendor/package"),
            "Vendor_Package_"
        );
    }

    #[test]
    fn test_minimal_config() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": {
                    "some/dep": "^1.0"
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.target_directory_name(), "vendor-prefixed");
        assert_eq!(config.vendor_directory_name(), "vendor");
        // namespace_prefix derived from package name
        assert_eq!(config.namespace_prefix(), Some("Test\\Project"));
        // classmap_prefix derived from package name
        assert_eq!(config.classmap_prefix(), Some("Test_Project_"));
        // packages derived from require
        assert_eq!(config.packages(), &["some/dep"]);
        assert!(!config.delete_vendor_files());
        assert!(!config.delete_vendor_packages());
        assert!(config.include_modified_date());
        assert!(config.include_author());
        assert!(!config.dry_run());
        assert!(!config.include_root_autoload());
    }

    #[test]
    fn test_strauss_extra_overrides() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "some/dep": "^1.0" },
                "extra": {
                    "strauss": {
                        "target_directory": "lib-prefixed",
                        "namespace_prefix": "MyPrefix\\",
                        "classmap_prefix": "MyPrefix_",
                        "constants_prefix": "MYPREFIX_",
                        "delete_vendor_packages": true,
                        "include_modified_date": false,
                        "include_author": false,
                        "include_root_autoload": true,
                        "packages": ["some/dep", "another/dep"]
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.target_directory_name(), "lib-prefixed");
        assert_eq!(config.namespace_prefix(), Some("MyPrefix"));
        assert_eq!(config.classmap_prefix(), Some("MyPrefix_"));
        assert_eq!(config.constants_prefix(), Some("MYPREFIX_"));
        assert!(config.delete_vendor_packages());
        assert!(!config.include_modified_date());
        assert!(!config.include_author());
        assert!(config.include_root_autoload());
        assert_eq!(config.packages(), &["some/dep", "another/dep"]);
    }

    #[test]
    fn test_mozart_backward_compat() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "some/dep": "^1.0" },
                "extra": {
                    "mozart": {
                        "dep_directory": "mozart-target",
                        "dep_namespace": "MozartNs\\",
                        "classmap_prefix": "MozartCm_",
                        "exclude_packages": ["excluded/pkg"]
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.target_directory_name(), "mozart-target");
        assert_eq!(config.namespace_prefix(), Some("MozartNs"));
        assert_eq!(config.classmap_prefix(), Some("MozartCm_"));
        assert_eq!(
            config.exclude_packages_from_prefixing(),
            &["excluded/pkg"]
        );
        // Mozart defaults to delete_vendor_files = true
        assert!(config.delete_vendor_files());
    }

    #[test]
    fn test_strauss_overrides_mozart() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "some/dep": "^1.0" },
                "extra": {
                    "mozart": {
                        "dep_directory": "mozart-dir",
                        "dep_namespace": "MozartNs\\"
                    },
                    "strauss": {
                        "target_directory": "strauss-dir",
                        "namespace_prefix": "StraussNs\\"
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        // Strauss should override Mozart
        assert_eq!(config.target_directory_name(), "strauss-dir");
        assert_eq!(config.namespace_prefix(), Some("StraussNs"));
    }

    #[test]
    fn test_namespace_inferred_from_psr4() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "autoload": {
                    "psr-4": {
                        "Test\\Project\\": "src/"
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.namespace_prefix(), Some("Test\\Project"));
        // classmap_prefix from psr-4 key
        assert_eq!(config.classmap_prefix(), Some("Test_Project_"));
    }

    #[test]
    fn test_functions_prefix_not_set_derives_from_classmap() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "classmap_prefix": "MyPrefix_"
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.get_functions_prefix(), Some("myprefix_".to_string()));
    }

    #[test]
    fn test_functions_prefix_disabled() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "functions_prefix": false
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.get_functions_prefix(), None);
    }

    #[test]
    fn test_functions_prefix_explicit_string() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "functions_prefix": "my_fn_prefix_"
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(
            config.get_functions_prefix(),
            Some("my_fn_prefix_".to_string())
        );
    }

    #[test]
    fn test_update_call_sites_from_string() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{ "name": "test/project", "require": { "dep/one": "^1.0" } }"#,
        );

        let mut config = StraussConfig::from_composer_json(&path).unwrap();

        // Default: disabled (empty vec)
        assert_eq!(config.update_call_sites(), Some([].as_slice()));

        // "true" => use autoload key (None)
        config.set_update_call_sites_from_string("true");
        assert_eq!(config.update_call_sites(), None);

        // "false" => disabled
        config.set_update_call_sites_from_string("false");
        assert_eq!(config.update_call_sites(), Some([].as_slice()));

        // specific paths
        config.set_update_call_sites_from_string("src,includes,lib");
        let paths = config.update_call_sites().unwrap();
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0], "src");
        assert_eq!(paths[1], "includes");
        assert_eq!(paths[2], "lib");
    }

    #[test]
    fn test_update_call_sites_json_true() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "update_call_sites": true
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        // true => None (use autoload)
        assert_eq!(config.update_call_sites(), None);
    }

    #[test]
    fn test_update_call_sites_json_false() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "update_call_sites": false
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.update_call_sites(), Some([].as_slice()));
    }

    #[test]
    fn test_update_call_sites_json_array() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "update_call_sites": ["src/", "includes/"]
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        let paths = config.update_call_sites().unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], "src/");
        assert_eq!(paths[1], "includes/");
    }

    #[test]
    fn test_exclude_configs() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "exclude_from_copy": {
                            "packages": ["excluded/copy-pkg"],
                            "namespaces": ["Excluded\\CopyNs"],
                            "file_patterns": ["/\\.txt$/"]
                        },
                        "exclude_from_prefix": {
                            "packages": ["excluded/prefix-pkg"],
                            "namespaces": ["Excluded\\PrefixNs\\"],
                            "file_patterns": ["/\\.md$/"]
                        }
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(
            config.exclude_packages_from_copy(),
            &["excluded/copy-pkg"]
        );
        assert_eq!(
            config.exclude_namespaces_from_copy(),
            &["Excluded\\CopyNs"]
        );
        assert_eq!(config.exclude_file_patterns_from_copy(), &["/\\.txt$/"]);

        assert_eq!(
            config.exclude_packages_from_prefixing(),
            &["excluded/prefix-pkg"]
        );
        // Namespaces are trimmed
        assert_eq!(
            config.exclude_namespaces_from_prefixing(),
            vec!["Excluded\\PrefixNs"]
        );
        assert_eq!(
            config.exclude_file_patterns_from_prefixing(),
            &["/\\.md$/"]
        );
    }

    #[test]
    fn test_create_aliases() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{ "name": "test/project", "require": { "dep/one": "^1.0" } }"#,
        );

        let mut config = StraussConfig::from_composer_json(&path).unwrap();
        assert!(!config.create_aliases());

        config.set_delete_vendor_packages(true);
        assert!(config.create_aliases());

        config.set_delete_vendor_packages(false);
        config.set_delete_vendor_files(true);
        assert!(config.create_aliases());

        config.set_delete_vendor_files(false);
        config.set_target_directory("vendor".to_string());
        assert!(config.create_aliases());
    }

    #[test]
    fn test_classmap_output_auto_detect() {
        let dir = TempDir::new().unwrap();
        // Target dir is NOT in autoload => classmap_output should be true
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "autoload": {
                    "psr-4": {
                        "Test\\Project\\": "src/"
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert!(config.classmap_output());
    }

    #[test]
    fn test_classmap_output_auto_detect_in_autoload() {
        let dir = TempDir::new().unwrap();
        // Target dir IS in autoload => classmap_output should be false
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "autoload": {
                    "classmap": ["vendor-prefixed/"]
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert!(!config.classmap_output());
    }

    #[test]
    fn test_dry_run() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{ "name": "test/project", "require": { "dep/one": "^1.0" } }"#,
        );

        let mut config = StraussConfig::from_composer_json(&path).unwrap();
        assert!(!config.dry_run());

        config.set_dry_run(true);
        assert!(config.dry_run());
        // project_directory should be prefixed with mem:/
        assert!(config.project_directory().starts_with("mem:/"));

        // raw project directory should not have the prefix
        assert!(!config.project_directory_raw().starts_with("mem:/"));
    }

    #[test]
    fn test_namespace_replacement_patterns() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "extra": {
                    "strauss": {
                        "namespace_replacement_patterns": {
                            "~BrianHenryIE\\\\(.*)~": "BrianHenryIE\\WC_Cash_App\\\\$1"
                        }
                    }
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        let patterns = config.namespace_replacement_patterns();
        assert_eq!(patterns.len(), 1);
        assert!(patterns.contains_key("~BrianHenryIE\\\\(.*)~"));
    }

    #[test]
    fn test_custom_vendor_dir() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "name": "test/project",
                "require": { "dep/one": "^1.0" },
                "config": {
                    "vendor-dir": "libs"
                }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.vendor_directory_name(), "libs");
    }

    #[test]
    fn test_classmap_prefix_inferred_from_namespace_prefix() {
        let dir = TempDir::new().unwrap();
        let path = write_composer_json(
            &dir,
            r#"{
                "extra": {
                    "strauss": {
                        "namespace_prefix": "MyVendor\\MyPlugin\\"
                    }
                },
                "require": { "dep/one": "^1.0" }
            }"#,
        );

        let config = StraussConfig::from_composer_json(&path).unwrap();
        assert_eq!(config.namespace_prefix(), Some("MyVendor\\MyPlugin"));
        // classmap_prefix should be derived from namespace_prefix
        assert!(config.classmap_prefix().is_some());
        let cp = config.classmap_prefix().unwrap();
        // Should have underscores instead of backslashes, ending with _
        assert!(cp.contains('_'));
        assert!(cp.ends_with('_'));
    }
}
