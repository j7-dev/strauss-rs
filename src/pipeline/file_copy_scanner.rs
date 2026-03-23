use anyhow::Result;
use log::debug;
use regex::Regex;

use crate::config::strauss_config::StraussConfig;
use crate::types::discovered_files::DiscoveredFiles;

/// Apply exclusion rules to discovered files.
/// Marks files with do_copy=false, do_prefix=false based on config.
///
/// Port of PHP FileCopyScanner
pub fn scan_files(
    files: &mut DiscoveredFiles,
    config: &StraussConfig,
) -> Result<()> {
    // If target == vendor, no copying needed (in-place mode)
    let in_place = config.target_directory == config.vendor_directory;

    let exclude_packages = &config.exclude_from_copy.packages;
    let exclude_namespaces = &config.exclude_from_copy.namespaces;
    let exclude_file_patterns: Vec<Regex> = config
        .exclude_from_copy
        .file_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    for file in files.files_mut() {
        // Check package exclusion
        if let Some(ref pkg_name) = file.package_name {
            if exclude_packages.contains(pkg_name) {
                file.do_copy = false;
                file.do_prefix = false;
                debug!("Excluding file (package): {}", file.vendor_relative_path);
                continue;
            }
        }

        // Check file pattern exclusion
        let mut excluded = false;
        for pattern in &exclude_file_patterns {
            if pattern.is_match(&file.vendor_relative_path) {
                file.do_copy = false;
                file.do_prefix = false;
                excluded = true;
                debug!("Excluding file (pattern): {}", file.vendor_relative_path);
                break;
            }
        }
        if excluded {
            continue;
        }

        // In-place mode: no copying needed
        if in_place {
            file.do_copy = false;
        }

        // Set delete flag based on config
        if config.delete_vendor_files && !is_symlink(&file.source_absolute_path) {
            file.do_delete = Some(true);
        }

        // If not copying, don't prefix (unless in-place mode)
        if !file.do_copy && !in_place {
            file.do_prefix = false;
        }
    }

    Ok(())
}

fn is_symlink(path: &std::path::Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}
