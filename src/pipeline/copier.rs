use anyhow::Result;
use log::{debug, info};
use rayon::prelude::*;
use std::fs;
use std::path::Path;

use crate::config::strauss_config::StraussConfig;
use crate::replacer::Replacer;
use crate::types::discovered_files::DiscoveredFiles;
use crate::types::symbols_collection::DiscoveredSymbols;

/// Copy files to target directory and apply replacements in a single pass.
///
/// This is the core performance optimization: instead of PHP's separate
/// copy → prefix → license passes, we do everything in one pass per file.
/// Files are processed in parallel using Rayon.
pub fn copy_and_prefix(
    files: &DiscoveredFiles,
    symbols: &DiscoveredSymbols,
    config: &StraussConfig,
    replacer: &Replacer,
) -> Result<()> {
    let all_files = files.files();
    let files_to_process: Vec<_> = all_files
        .iter()
        .filter(|f| f.do_copy)
        .collect();

    info!("Processing {} files (copy + prefix)...", files_to_process.len());

    // Process files in parallel
    let errors: Vec<_> = files_to_process
        .par_iter()
        .filter_map(|file| {
            match process_single_file(file, symbols, config, replacer) {
                Ok(_) => None,
                Err(e) => Some(format!("{}: {}", file.vendor_relative_path, e)),
            }
        })
        .collect();

    if !errors.is_empty() {
        for error in &errors {
            log::warn!("Error processing file: {}", error);
        }
    }

    Ok(())
}

/// Process a single file: read → replace → write to target.
fn process_single_file(
    file: &crate::types::file::DiscoveredFile,
    symbols: &DiscoveredSymbols,
    config: &StraussConfig,
    replacer: &Replacer,
) -> Result<()> {
    // Ensure target directory exists
    if let Some(parent) = file.absolute_target_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if file.is_php_file() && file.do_prefix {
        // Read, replace, write
        let contents = fs::read_to_string(&file.source_absolute_path)?;
        let updated = replacer.replace_in_string(&contents, symbols)?;

        fs::write(&file.absolute_target_path, &updated)?;

        if updated != contents {
            debug!("Updated: {}", file.vendor_relative_path);
        }
    } else {
        // Non-PHP file or excluded from prefix: just copy
        fs::copy(&file.source_absolute_path, &file.absolute_target_path)?;
    }

    Ok(())
}
