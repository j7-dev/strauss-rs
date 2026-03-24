use anyhow::Result;
use log::{debug, info};
use std::path::Path;
use walkdir::WalkDir;

use crate::config::strauss_config::StraussConfig;
use crate::config::composer_package::ComposerPackage;
use crate::types::discovered_files::DiscoveredFiles;
use crate::types::file::DiscoveredFile;

/// Enumerate all files from dependency packages.
///
/// Port of PHP FileEnumerator::compileFileListForDependencies()
pub fn enumerate_files(
    working_dir: &Path,
    config: &StraussConfig,
    dependencies: &[ComposerPackage],
) -> Result<DiscoveredFiles> {
    let mut files = DiscoveredFiles::new();
    let vendor_dir = working_dir.join(&config.vendor_directory);
    let target_dir = working_dir.join(&config.target_directory);

    for package in dependencies {
        let package_dir = vendor_dir.join(&package.name);
        if !package_dir.exists() {
            debug!("Package directory does not exist: {}", package_dir.display());
            continue;
        }

        // Walk all files in the package directory
        for entry in WalkDir::new(&package_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() {
                continue;
            }

            let source_path = entry.path().to_path_buf();
            let vendor_relative = source_path
                .strip_prefix(&vendor_dir)
                .unwrap_or(&source_path)
                .to_string_lossy()
                .replace('\\', "/");

            let package_relative = source_path
                .strip_prefix(&package_dir)
                .unwrap_or(&source_path)
                .to_string_lossy()
                .replace('\\', "/");

            let target_path = target_dir.join(&vendor_relative);

            let file = DiscoveredFile {
                source_absolute_path: source_path,
                vendor_relative_path: vendor_relative,
                absolute_target_path: target_path,
                do_copy: true,
                do_prefix: true,
                do_delete: None,
                did_delete: false,
                did_update: false,
                is_autoloaded: false,
                autoloader_types: Vec::new(),
                package_name: Some(package.name.clone()),
                package_relative_path: Some(package_relative),
                discovered_symbols: Vec::new(),
                file_references: None,
            };

            files.add(file);
        }
    }

    Ok(files)
}
