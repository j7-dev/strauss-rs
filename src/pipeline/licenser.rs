use anyhow::Result;
use log::debug;
use std::fs;
use std::path::Path;

use crate::config::strauss_config::StraussConfig;
use crate::config::composer_package::ComposerPackage;
use crate::types::discovered_files::DiscoveredFiles;

const LICENSE_FILE_NAMES: &[&str] = &[
    "LICENSE",
    "LICENSE.txt",
    "LICENSE.md",
    "LICENCE",
    "LICENCE.txt",
    "LICENCE.md",
    "COPYING",
    "COPYING.txt",
    "NOTICE",
    "NOTICE.txt",
];

/// Copy license files and add modification headers.
///
/// Port of PHP Licenser
pub fn add_licenses(
    files: &DiscoveredFiles,
    dependencies: &[ComposerPackage],
    config: &StraussConfig,
) -> Result<()> {
    // License headers are already handled during the copy+prefix phase
    // This function handles copying standalone license files

    let target_dir = Path::new(&config.target_directory);

    for package in dependencies {
        let vendor_pkg_dir = Path::new(&config.vendor_directory).join(&package.name);

        for license_name in LICENSE_FILE_NAMES {
            let license_source = vendor_pkg_dir.join(license_name);
            if license_source.exists() {
                let license_target = target_dir.join(&package.name).join(license_name);
                if let Some(parent) = license_target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&license_source, &license_target)?;
                debug!("Copied license: {}/{}", package.name, license_name);
                break; // Only copy the first found license file
            }
        }
    }

    Ok(())
}

/// Generate a PHP doc modification header.
pub fn modification_header(author: &str, include_date: bool, include_author: bool) -> String {
    let mut parts = Vec::new();

    if include_author {
        parts.push(format!(" * Modified by {}", author));
    }

    if include_date {
        let date = chrono_like_date();
        parts.push(format!(" * on {}", date));
    }

    parts.push(" * using strauss-rs".to_string());

    if parts.is_empty() {
        return String::new();
    }

    format!("/**\n{}\n */\n", parts.join("\n"))
}

fn chrono_like_date() -> String {
    // Simple date without pulling in chrono dependency
    use std::time::SystemTime;
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Simple epoch to date conversion
    let days = now / 86400;
    let years = days / 365;
    let year = 1970 + years;
    let remaining_days = days % 365;
    let month = remaining_days / 30 + 1;
    let day = remaining_days % 30 + 1;

    format!("{:04}-{:02}-{:02}", year, month, day)
}
