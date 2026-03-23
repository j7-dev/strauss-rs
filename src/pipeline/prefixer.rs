use anyhow::Result;
use log::{debug, info};
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

use crate::replacer::Replacer;
use crate::types::symbols_collection::DiscoveredSymbols;

/// Replace prefixed references in project files (update call sites).
///
/// Port of PHP Prefixer::replaceInProjectFiles()
pub fn replace_in_project_files(
    working_dir: &Path,
    call_sites: &[String],
    symbols: &DiscoveredSymbols,
    replacer: &Replacer,
) -> Result<()> {
    // Collect all PHP files from the specified directories
    let mut php_files: Vec<std::path::PathBuf> = Vec::new();

    for site in call_sites {
        let site_path = working_dir.join(site);
        if site_path.is_file() {
            if site_path.extension().map_or(false, |e| e == "php") {
                php_files.push(site_path);
            }
        } else if site_path.is_dir() {
            for entry in WalkDir::new(&site_path)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                if entry.file_type().is_file()
                    && entry.path().extension().map_or(false, |e| e == "php")
                {
                    php_files.push(entry.path().to_path_buf());
                }
            }
        }
    }

    info!("Updating {} project files...", php_files.len());

    // Process in parallel
    php_files.par_iter().for_each(|path| {
        if let Ok(contents) = fs::read_to_string(path) {
            if let Ok(updated) = replacer.replace_in_string(&contents, symbols) {
                if updated != contents {
                    let _ = fs::write(path, &updated);
                    debug!("Updated project file: {}", path.display());
                }
            }
        }
    });

    Ok(())
}
