use anyhow::Result;
use dashmap::DashMap;
use log::{debug, info};
use rayon::prelude::*;
use std::sync::Arc;

use crate::config::strauss_config::StraussConfig;
use crate::parser::php_parser::PhpParser;
use crate::parser::symbol_extractor;
use crate::parser::reference_extractor;
use crate::parser::builtins;
use crate::types::discovered_files::DiscoveredFiles;
use crate::types::file_references::FileReferences;
use crate::types::symbol::{DiscoveredSymbol, SymbolType};
use crate::types::symbols_collection::DiscoveredSymbols;

/// Scan all PHP files for symbols using tree-sitter (parallel).
/// Also extracts per-file references for optimized Phase 3 filtering.
///
/// Port of PHP FileSymbolScanner::findInFiles()
/// Major performance improvement: parallel parsing with Rayon.
pub fn scan_files_for_symbols(
    files: &DiscoveredFiles,
    config: &StraussConfig,
) -> Result<(DiscoveredSymbols, DashMap<String, FileReferences>)> {
    let symbols = DiscoveredSymbols::new();
    let file_refs: DashMap<String, FileReferences> = DashMap::new();

    // Collect PHP files that need scanning
    let all_files = files.files();
    let php_files: Vec<_> = all_files
        .iter()
        .filter(|f| f.is_php_file() && f.is_autoloaded)
        .collect();

    info!("Scanning {} PHP files for symbols...", php_files.len());

    // Process files in parallel using Rayon
    // Each thread gets its own parser instance (tree-sitter Parser is not Send)
    let symbols_ref = &symbols;
    let file_refs_ref = &file_refs;

    php_files.par_iter().for_each(|file| {
        let result = scan_single_file(file, symbols_ref, file_refs_ref);
        if let Err(e) = result {
            debug!("Error scanning {}: {}", file.vendor_relative_path, e);
        }
    });

    // Also extract references from ALL PHP files that will be copied
    // (not just autoloaded ones), since they also need replacement
    let copy_php_files: Vec<_> = all_files
        .iter()
        .filter(|f| f.is_php_file() && f.do_copy && !f.is_autoloaded)
        .collect();

    if !copy_php_files.is_empty() {
        info!("Extracting references from {} additional PHP files...", copy_php_files.len());
        copy_php_files.par_iter().for_each(|file| {
            let result = extract_refs_only(file, file_refs_ref);
            if let Err(e) = result {
                debug!("Error extracting refs from {}: {}", file.vendor_relative_path, e);
            }
        });
    }

    Ok((symbols, file_refs))
}

/// Extract only references (no symbol declarations) from a PHP file.
/// Used for files that need replacement but aren't autoloaded.
fn extract_refs_only(
    file: &crate::types::file::DiscoveredFile,
    file_refs: &DashMap<String, FileReferences>,
) -> Result<()> {
    let contents = std::fs::read_to_string(&file.source_absolute_path)?;
    let mut parser = PhpParser::new()?;
    let tree = parser.parse(&contents)?;
    let refs = reference_extractor::extract_references(&tree, &contents);
    let key = file.source_absolute_path.to_string_lossy().to_string();
    file_refs.insert(key, refs);
    Ok(())
}

/// Scan a single PHP file for symbols and extract references.
fn scan_single_file(
    file: &crate::types::file::DiscoveredFile,
    symbols: &DiscoveredSymbols,
    file_refs: &DashMap<String, FileReferences>,
) -> Result<()> {
    let contents = std::fs::read_to_string(&file.source_absolute_path)?;

    let mut parser = PhpParser::new()?;
    let tree = parser.parse(&contents)?;

    let extracted = symbol_extractor::extract_symbols(&tree, &contents);

    // Extract per-file references (reuses the same tree, near-zero cost)
    let refs = reference_extractor::extract_references(&tree, &contents);
    let key = file.source_absolute_path.to_string_lossy().to_string();
    file_refs.insert(key, refs);

    let package_name = file.package_name.clone();
    let source_path = file.source_absolute_path.to_string_lossy().to_string();

    // Add discovered namespaces
    for ns in extracted.namespaces {
        if builtins::is_builtin_class(&ns.name) {
            continue;
        }
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Namespace,
            original_symbol: ns.name,
            replacement: None,
            namespace: None,
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        });
    }

    // Add discovered classes
    for class in extracted.classes {
        if builtins::is_builtin_class(&class.name) {
            continue;
        }
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Class,
            original_symbol: if class.namespace.is_empty() {
                class.name.clone()
            } else {
                format!("{}\\{}", class.namespace, class.name)
            },
            replacement: None,
            namespace: if class.namespace.is_empty() { None } else { Some(class.namespace) },
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: class.extends,
            interfaces: class.implements,
            is_abstract: class.is_abstract,
        });
    }

    // Add interfaces
    for iface in extracted.interfaces {
        if builtins::is_builtin_class(&iface.name) {
            continue;
        }
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Interface,
            original_symbol: if iface.namespace.is_empty() {
                iface.name.clone()
            } else {
                format!("{}\\{}", iface.namespace, iface.name)
            },
            replacement: None,
            namespace: if iface.namespace.is_empty() { None } else { Some(iface.namespace) },
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: None,
            interfaces: iface.extends,
            is_abstract: false,
        });
    }

    // Add traits
    for tr in extracted.traits {
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Trait,
            original_symbol: if tr.namespace.is_empty() {
                tr.name.clone()
            } else {
                format!("{}\\{}", tr.namespace, tr.name)
            },
            replacement: None,
            namespace: if tr.namespace.is_empty() { None } else { Some(tr.namespace) },
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        });
    }

    // Add functions (global only)
    for func in extracted.functions {
        if builtins::is_builtin_function(&func.name) {
            continue;
        }
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Function,
            original_symbol: func.name,
            replacement: None,
            namespace: if func.namespace.is_empty() { None } else { Some(func.namespace) },
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        });
    }

    // Add constants
    for constant in extracted.constants {
        symbols.add(DiscoveredSymbol {
            symbol_type: SymbolType::Constant,
            original_symbol: constant.name,
            replacement: None,
            namespace: if constant.namespace.is_empty() { None } else { Some(constant.namespace) },
            do_rename: true,
            source_files: vec![source_path.clone()],
            package_name: package_name.clone(),
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        });
    }

    Ok(())
}
