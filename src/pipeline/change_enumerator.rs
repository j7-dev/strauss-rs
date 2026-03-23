use anyhow::Result;
use log::debug;
use regex::Regex;

use crate::config::strauss_config::StraussConfig;
use crate::types::symbol::SymbolType;
use crate::types::symbols_collection::DiscoveredSymbols;

/// Determine the replacement name for each symbol.
///
/// Port of PHP ChangeEnumerator::determineReplacements()
pub fn determine_replacements(
    symbols: &mut DiscoveredSymbols,
    config: &StraussConfig,
) -> Result<()> {
    let namespace_prefix = config.namespace_prefix.as_deref();
    let classmap_prefix = config.classmap_prefix.as_deref();
    let functions_prefix = config.functions_prefix.as_deref();
    let constants_prefix = config.constants_prefix.as_deref();

    // Compile namespace_replacement_patterns
    let replacement_patterns: Vec<(Regex, String)> = config
        .namespace_replacement_patterns
        .iter()
        .filter_map(|(pattern, replacement)| {
            Regex::new(pattern).ok().map(|r| (r, replacement.clone()))
        })
        .collect();

    let mut all_symbols = symbols.all_symbols_mut();

    for symbol in all_symbols.iter_mut() {
        if !symbol.do_rename {
            continue;
        }

        match symbol.symbol_type {
            SymbolType::Namespace => {
                let mut matched = false;
                for (pattern, replacement_template) in &replacement_patterns {
                    if pattern.is_match(&symbol.original_symbol) {
                        let replacement = pattern
                            .replace(&symbol.original_symbol, replacement_template.as_str())
                            .to_string();
                        symbol.replacement = Some(replacement);
                        matched = true;
                        break;
                    }
                }

                if !matched {
                    if let Some(prefix) = namespace_prefix {
                        let prefix_trimmed = prefix.trim_end_matches('\\');
                        symbol.replacement = Some(format!(
                            "{}\\{}",
                            prefix_trimmed,
                            symbol.original_symbol
                        ));
                    } else {
                        symbol.do_rename = false;
                    }
                }
            }

            SymbolType::Class | SymbolType::Interface | SymbolType::Trait => {
                if symbol.namespace.is_some() {
                    // Namespaced: handled by namespace replacement
                    symbol.replacement = Some(symbol.original_symbol.clone());
                } else {
                    if let Some(prefix) = classmap_prefix {
                        if !symbol.original_symbol.starts_with(prefix) {
                            symbol.replacement = Some(format!("{}{}", prefix, symbol.original_symbol));
                        } else {
                            symbol.do_rename = false;
                        }
                    } else {
                        symbol.do_rename = false;
                    }
                }
            }

            SymbolType::Function => {
                if let Some(prefix) = functions_prefix {
                    if !symbol.original_symbol.starts_with(prefix) {
                        symbol.replacement = Some(format!("{}{}", prefix, symbol.original_symbol));
                    } else {
                        symbol.do_rename = false;
                    }
                } else {
                    symbol.do_rename = false;
                }
            }

            SymbolType::Constant => {
                if let Some(prefix) = constants_prefix {
                    if !symbol.original_symbol.starts_with(prefix) {
                        symbol.replacement = Some(format!("{}{}", prefix, symbol.original_symbol));
                    } else {
                        symbol.do_rename = false;
                    }
                } else {
                    symbol.do_rename = false;
                }
            }
        }
    }

    symbols.update_symbols(all_symbols);
    Ok(())
}
