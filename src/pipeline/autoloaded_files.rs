use anyhow::Result;
use log::debug;

use crate::config::composer_package::ComposerPackage;
use crate::types::discovered_files::DiscoveredFiles;

/// Mark files as autoloaded based on package autoload configuration.
///
/// Port of PHP AutoloadedFilesEnumerator
pub fn enumerate_autoloaded_files(
    files: &mut DiscoveredFiles,
    dependencies: &[ComposerPackage],
) -> Result<()> {
    for package in dependencies {
        // PSR-4 autoloader
        for (namespace, paths) in &package.autoload_psr4 {
            for path in paths {
                let prefix = format!("{}/{}", package.name, path.trim_end_matches('/'));
                for file in files.files_mut() {
                    if file.vendor_relative_path.starts_with(&prefix) && file.is_php_file() {
                        file.is_autoloaded = true;
                        if !file.autoloader_types.contains(&"psr-4".to_string()) {
                            file.autoloader_types.push("psr-4".to_string());
                        }
                    }
                }
            }
        }

        // PSR-0 autoloader
        for (namespace, paths) in &package.autoload_psr0 {
            for path in paths {
                let prefix = format!("{}/{}", package.name, path.trim_end_matches('/'));
                for file in files.files_mut() {
                    if file.vendor_relative_path.starts_with(&prefix) && file.is_php_file() {
                        file.is_autoloaded = true;
                        if !file.autoloader_types.contains(&"psr-0".to_string()) {
                            file.autoloader_types.push("psr-0".to_string());
                        }
                    }
                }
            }
        }

        // Classmap autoloader
        for path in &package.autoload_classmap {
            let prefix = format!("{}/{}", package.name, path.trim_end_matches('/'));
            for file in files.files_mut() {
                if file.vendor_relative_path.starts_with(&prefix) && file.is_php_file() {
                    file.is_autoloaded = true;
                    if !file.autoloader_types.contains(&"classmap".to_string()) {
                        file.autoloader_types.push("classmap".to_string());
                    }
                }
            }
        }

        // Files autoloader
        for file_path in &package.autoload_files {
            let target = format!("{}/{}", package.name, file_path);
            for file in files.files_mut() {
                if file.vendor_relative_path == target {
                    file.is_autoloaded = true;
                    if !file.autoloader_types.contains(&"files".to_string()) {
                        file.autoloader_types.push("files".to_string());
                    }
                }
            }
        }
    }

    Ok(())
}
