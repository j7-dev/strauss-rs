use anyhow::Result;
use log::debug;
use regex::Regex;

use crate::config::strauss_config::StraussConfig;
use crate::types::discovered_files::DiscoveredFiles;
use crate::types::symbol::SymbolType;
use crate::types::symbols_collection::DiscoveredSymbols;

/// Mark which symbols should be renamed based on exclusion rules.
///
/// Port of PHP MarkSymbolsForRenaming
pub fn mark_symbols_for_renaming(
    symbols: &mut DiscoveredSymbols,
    files: &DiscoveredFiles,
    config: &StraussConfig,
) -> Result<()> {
    let exclude_packages = &config.exclude_from_prefix.packages;
    let exclude_namespaces = &config.exclude_from_prefix.namespaces;
    let exclude_file_patterns: Vec<Regex> = config
        .exclude_from_prefix
        .file_patterns
        .iter()
        .filter_map(|p| Regex::new(p).ok())
        .collect();

    // Collect all symbols, mutate, then write back
    let mut all_symbols = symbols.all_symbols_mut();

    for symbol in all_symbols.iter_mut() {
        // Check package exclusion
        if let Some(ref pkg) = symbol.package_name {
            if exclude_packages.contains(pkg) {
                symbol.do_rename = false;
                continue;
            }
        }

        // Check namespace exclusion
        if let Some(ref ns) = symbol.namespace {
            if exclude_namespaces.contains(ns) {
                symbol.do_rename = false;
                continue;
            }
        }

        // Check if original_symbol itself is an excluded namespace
        if exclude_namespaces.contains(&symbol.original_symbol) {
            symbol.do_rename = false;
            continue;
        }

        // Check file pattern exclusion
        let mut pattern_excluded = false;
        for source_file in &symbol.source_files {
            for pattern in &exclude_file_patterns {
                if pattern.is_match(source_file) {
                    pattern_excluded = true;
                    break;
                }
            }
            if pattern_excluded { break; }
        }
        if pattern_excluded {
            symbol.do_rename = false;
            continue;
        }

        // Check if the symbol's file will be copied
        let in_copied_file = symbol.source_files.iter().any(|sf| {
            files.files().iter().any(|f| {
                f.source_absolute_path.to_string_lossy() == *sf && f.do_copy
            })
        });

        // Namespace symbols should always pass
        if !in_copied_file && symbol.symbol_type != SymbolType::Namespace {
            symbol.do_rename = false;
        }
    }

    symbols.update_symbols(all_symbols);
    Ok(())
}
