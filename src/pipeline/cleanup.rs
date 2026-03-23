use anyhow::Result;
use log::{debug, info};
use std::fs;
use std::path::Path;

use crate::config::strauss_config::StraussConfig;
use crate::config::composer_package::ComposerPackage;
use crate::types::discovered_files::DiscoveredFiles;

/// Clean up vendor files/packages after processing.
///
/// Port of PHP Cleanup
pub fn cleanup(
    files: &DiscoveredFiles,
    dependencies: &[ComposerPackage],
    config: &StraussConfig,
) -> Result<()> {
    if config.delete_vendor_packages {
        delete_vendor_packages(dependencies, config)?;
    } else if config.delete_vendor_files {
        delete_vendor_files(files)?;
    }

    Ok(())
}

fn delete_vendor_packages(
    dependencies: &[ComposerPackage],
    config: &StraussConfig,
) -> Result<()> {
    for package in dependencies {
        let package_dir = Path::new(&config.vendor_directory).join(&package.name);

        if !package_dir.exists() {
            continue;
        }

        // Don't delete symlinked directories
        if package_dir.symlink_metadata()?.file_type().is_symlink() {
            debug!("Skipping symlinked package: {}", package.name);
            continue;
        }

        fs::remove_dir_all(&package_dir)?;
        info!("Deleted vendor package: {}", package.name);

        // Clean up empty parent directories
        if let Some(parent) = package_dir.parent() {
            if parent.exists() && is_dir_empty(parent)? {
                let _ = fs::remove_dir(parent);
            }
        }
    }

    Ok(())
}

fn delete_vendor_files(files: &DiscoveredFiles) -> Result<()> {
    for file in files.files() {
        if file.do_delete.unwrap_or(false) && !file.did_delete {
            if file.source_absolute_path.exists() {
                // Don't delete symlinks
                if file.source_absolute_path.symlink_metadata()?.file_type().is_symlink() {
                    continue;
                }
                fs::remove_file(&file.source_absolute_path)?;
                debug!("Deleted: {}", file.vendor_relative_path);
            }
        }
    }

    Ok(())
}

fn is_dir_empty(path: &Path) -> Result<bool> {
    Ok(fs::read_dir(path)?.next().is_none())
}
