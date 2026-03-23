pub mod namespace_replacer;
pub mod class_replacer;
pub mod function_replacer;
pub mod constant_replacer;
pub mod string_literal;
pub mod regex_cache;

use anyhow::Result;
use log::{debug, info};

use crate::types::symbol::DiscoveredSymbol;
use crate::types::symbols_collection::DiscoveredSymbols;
use self::namespace_replacer::NamespaceReplacer;
use self::class_replacer::ClassReplacer;
use self::function_replacer::FunctionReplacer;
use self::constant_replacer::ConstantReplacer;

/// Pre-computed replacement data, collected once and shared across all files.
pub struct ReplacementData {
    pub namespace_changes: Vec<(String, String)>,
    pub global_classes: Vec<DiscoveredSymbol>,
    pub constants: Vec<String>,
    pub functions: Vec<(String, String)>,  // (original, replacement)
}

/// The main replacement engine. Orchestrates all replacement types.
/// Mirrors PHP Prefixer::replaceInString()
pub struct Replacer {
    namespace_replacer: NamespaceReplacer,
    class_replacer: ClassReplacer,
    function_replacer: FunctionReplacer,
    constant_replacer: ConstantReplacer,
    namespace_prefix: Option<String>,
    classmap_prefix: Option<String>,
    constants_prefix: Option<String>,
    exclude_namespaces: Vec<String>,
    /// Pre-computed data (populated once, reused for every file)
    data: Option<ReplacementData>,
}

impl Replacer {
    pub fn new(
        namespace_prefix: Option<String>,
        classmap_prefix: Option<String>,
        constants_prefix: Option<String>,
        exclude_namespaces: Vec<String>,
    ) -> Self {
        Self {
            namespace_replacer: NamespaceReplacer::new(),
            class_replacer: ClassReplacer::new(),
            function_replacer: FunctionReplacer::new(),
            constant_replacer: ConstantReplacer::new(),
            namespace_prefix,
            classmap_prefix,
            constants_prefix,
            exclude_namespaces,
            data: None,
        }
    }

    /// Pre-compute all replacement data from symbols. Call ONCE before processing files.
    pub fn prepare(&mut self, symbols: &DiscoveredSymbols) {
        let namespace_symbols = symbols.get_namespaces();

        let namespace_changes: Vec<(String, String)> = namespace_symbols
            .iter()
            .filter(|s| s.do_rename)
            .filter(|s| !self.exclude_namespaces.contains(&s.original_symbol))
            .map(|s| (s.original_symbol.clone(), s.get_replacement().to_string()))
            .collect();

        let global_classes: Vec<DiscoveredSymbol> = if self.classmap_prefix.is_some() {
            symbols.get_global_classes()
                .into_iter()
                .filter(|c| c.do_rename)
                .collect()
        } else {
            Vec::new()
        };

        let constants: Vec<String> = if self.constants_prefix.is_some() {
            symbols.get_constants()
                .iter()
                .filter(|s| s.do_rename)
                .map(|s| s.original_symbol.clone())
                .collect()
        } else {
            Vec::new()
        };

        let functions: Vec<(String, String)> = symbols.get_functions()
            .iter()
            .filter(|f| f.do_rename && f.original_symbol != f.get_replacement())
            .map(|f| (f.original_symbol.clone(), f.get_replacement().to_string()))
            .collect();

        // Sort namespace changes shortest first (matching PHP behavior)
        let mut ns = namespace_changes;
        ns.sort_by(|a, b| a.0.len().cmp(&b.0.len()));

        self.data = Some(ReplacementData {
            namespace_changes: ns,
            global_classes,
            constants,
            functions,
        });
    }

    /// Apply all replacements to a string. Uses pre-computed data.
    pub fn replace_in_string(
        &self,
        contents: &str,
        symbols: &DiscoveredSymbols,
    ) -> Result<String> {
        let data = self.data.as_ref().expect("Call prepare() before replace_in_string()");
        let mut result = contents.to_string();

        // 1. Replace global classnames (classmap_prefix)
        if let Some(ref prefix) = self.classmap_prefix {
            for class in &data.global_classes {
                result = self.class_replacer.replace_classname(
                    &result,
                    &class.original_symbol,
                    prefix,
                )?;
            }
        }

        // 2. Replace ALL namespaces in a single Aho-Corasick pass
        result = self.namespace_replacer.replace_all_namespaces(
            &result,
            &data.namespace_changes,
        )?;

        // 3. Replace constants (single-pass Aho-Corasick)
        if let Some(ref prefix) = self.constants_prefix {
            if !data.constants.is_empty() {
                result = self.constant_replacer.replace_constants(
                    &result,
                    &data.constants,
                    prefix,
                )?;
            }
        }

        // 4. Replace functions using simple string replacement
        // (tree-sitter approach was too slow - creating parser per function per file)
        for (original, replacement) in &data.functions {
            // Simple word-boundary replacement for function names
            let pattern = format!(r"\b{}\b", regex::escape(original));
            if let Ok(re) = regex::Regex::new(&pattern) {
                result = re.replace_all(&result, replacement.as_str()).to_string();
            }
        }

        Ok(result)
    }
}
