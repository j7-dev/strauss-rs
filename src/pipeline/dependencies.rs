use anyhow::Result;
use log::{debug, info};
use std::collections::HashMap;
use std::path::Path;

use crate::config::strauss_config::StraussConfig;
use crate::config::composer_package::ComposerPackage;
use crate::config::composer_json::{ComposerJson, ComposerLock};

/// Enumerate all dependencies from composer.json and composer.lock.
///
/// Port of PHP DependenciesEnumerator::getAllDependencies()
pub fn enumerate_dependencies(
    working_dir: &Path,
    config: &StraussConfig,
) -> Result<Vec<ComposerPackage>> {
    let composer_json_path = working_dir.join("composer.json");
    let composer_lock_path = working_dir.join("composer.lock");

    // Read project composer.json
    let project_json: ComposerJson = {
        let content = std::fs::read_to_string(&composer_json_path)?;
        serde_json::from_str(&content)?
    };

    // Read composer.lock for resolved versions
    let lock: Option<ComposerLock> = if composer_lock_path.exists() {
        let content = std::fs::read_to_string(&composer_lock_path)?;
        Some(serde_json::from_str(&content)?)
    } else {
        None
    };

    // Determine which packages to process
    let package_names: Vec<String> = if config.packages.is_empty() {
        // Use all requires from composer.json
        project_json
            .require
            .as_ref()
            .map(|r| r.keys().cloned().collect())
            .unwrap_or_default()
    } else {
        config.packages.clone()
    };

    let vendor_dir = working_dir.join(&config.vendor_directory);

    // Build the flat dependency tree
    let mut result: HashMap<String, ComposerPackage> = HashMap::new();
    let lock_packages: HashMap<String, &crate::config::composer_json::ComposerLockPackage> = lock
        .as_ref()
        .map(|l| {
            l.packages
                .iter()
                .map(|p| (p.name.clone(), p))
                .collect()
        })
        .unwrap_or_default();

    recursive_get_dependencies(
        &package_names,
        &vendor_dir,
        &lock_packages,
        &config.override_autoload,
        &mut result,
    )?;

    Ok(result.into_values().collect())
}

fn recursive_get_dependencies(
    package_names: &[String],
    vendor_dir: &Path,
    lock_packages: &HashMap<String, &crate::config::composer_json::ComposerLockPackage>,
    override_autoload: &HashMap<String, serde_json::Value>,
    result: &mut HashMap<String, ComposerPackage>,
) -> Result<()> {
    for name in package_names {
        // Skip PHP platform requirements and already-processed packages
        if name.starts_with("php") || name.starts_with("ext-") || result.contains_key(name) {
            continue;
        }

        let package_dir = vendor_dir.join(name);

        // Try to load from vendor directory first
        let package = if package_dir.join("composer.json").exists() {
            ComposerPackage::from_vendor_dir(&package_dir, name, override_autoload)?
        } else if let Some(lock_pkg) = lock_packages.get(name) {
            // Fall back to composer.lock data
            ComposerPackage::from_lock_package(lock_pkg, vendor_dir, override_autoload)?
        } else {
            debug!("Skipping package not found: {}", name);
            continue;
        };

        // Get this package's requires for recursive resolution
        let sub_requires: Vec<String> = package.requires.clone();

        result.insert(name.clone(), package);

        // Recursively resolve sub-dependencies
        if !sub_requires.is_empty() {
            recursive_get_dependencies(
                &sub_requires,
                vendor_dir,
                lock_packages,
                override_autoload,
                result,
            )?;
        }
    }

    Ok(())
}
