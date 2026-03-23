use dashmap::DashMap;

use super::symbol::{DiscoveredSymbol, SymbolType};

/// Thread-safe collection of discovered PHP symbols, grouped by type.
///
/// Uses `DashMap` for lock-free concurrent access during parallel parsing with Rayon.
/// Symbols are keyed by their original fully-qualified name within each type-specific map.
#[derive(Debug)]
pub struct DiscoveredSymbols {
    namespaces: DashMap<String, DiscoveredSymbol>,
    classes: DashMap<String, DiscoveredSymbol>,
    interfaces: DashMap<String, DiscoveredSymbol>,
    traits: DashMap<String, DiscoveredSymbol>,
    functions: DashMap<String, DiscoveredSymbol>,
    constants: DashMap<String, DiscoveredSymbol>,
}

impl Clone for DiscoveredSymbols {
    fn clone(&self) -> Self {
        let clone_map =
            |map: &DashMap<String, DiscoveredSymbol>| -> DashMap<String, DiscoveredSymbol> {
                let new_map = DashMap::new();
                for entry in map.iter() {
                    new_map.insert(entry.key().clone(), entry.value().clone());
                }
                new_map
            };
        Self {
            namespaces: clone_map(&self.namespaces),
            classes: clone_map(&self.classes),
            interfaces: clone_map(&self.interfaces),
            traits: clone_map(&self.traits),
            functions: clone_map(&self.functions),
            constants: clone_map(&self.constants),
        }
    }
}

impl Default for DiscoveredSymbols {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveredSymbols {
    /// Create a new empty collection.
    pub fn new() -> Self {
        Self {
            namespaces: DashMap::new(),
            classes: DashMap::new(),
            interfaces: DashMap::new(),
            traits: DashMap::new(),
            functions: DashMap::new(),
            constants: DashMap::new(),
        }
    }

    /// Get a reference to the appropriate map for a given symbol type.
    fn map_for_type(&self, symbol_type: &SymbolType) -> &DashMap<String, DiscoveredSymbol> {
        match symbol_type {
            SymbolType::Namespace => &self.namespaces,
            SymbolType::Class => &self.classes,
            SymbolType::Interface => &self.interfaces,
            SymbolType::Trait => &self.traits,
            SymbolType::Function => &self.functions,
            SymbolType::Constant => &self.constants,
        }
    }

    /// Add a symbol to the appropriate type-specific map.
    ///
    /// If a symbol with the same original name already exists, the source files
    /// from the new symbol are merged into the existing one.
    pub fn add(&self, symbol: DiscoveredSymbol) {
        let map = self.map_for_type(&symbol.symbol_type);
        let key = symbol.original_symbol.clone();

        map.entry(key)
            .and_modify(|existing| {
                // Merge source files from the new symbol into the existing one.
                for source_file in &symbol.source_files {
                    existing.add_source_file(source_file.clone());
                }
            })
            .or_insert(symbol);
    }

    /// Get all namespace symbols as a Vec.
    pub fn get_namespaces(&self) -> Vec<DiscoveredSymbol> {
        self.namespaces
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get only classes in the global namespace (namespace is `\` or empty).
    ///
    /// Mirrors the PHP `getGlobalClasses()` method.
    pub fn get_classes(&self) -> Vec<DiscoveredSymbol> {
        self.classes
            .iter()
            .filter(|entry| entry.value().is_global_namespace())
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all classes regardless of namespace.
    ///
    /// Mirrors the PHP `getAllClasses()` method.
    pub fn get_all_classes(&self) -> Vec<DiscoveredSymbol> {
        self.classes
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all function symbols.
    pub fn get_functions(&self) -> Vec<DiscoveredSymbol> {
        self.functions
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all constant symbols.
    pub fn get_constants(&self) -> Vec<DiscoveredSymbol> {
        self.constants
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all trait symbols.
    pub fn get_traits(&self) -> Vec<DiscoveredSymbol> {
        self.traits
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all interface symbols.
    pub fn get_interfaces(&self) -> Vec<DiscoveredSymbol> {
        self.interfaces
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all symbols across all types.
    ///
    /// Note: The PHP `getSymbols()` method returns namespaces + global classes + constants + functions.
    /// This method returns everything, including namespaced classes, interfaces, and traits.
    pub fn get_all_symbols(&self) -> Vec<DiscoveredSymbol> {
        let mut all = Vec::new();
        all.extend(self.get_namespaces());
        all.extend(self.get_all_classes());
        all.extend(self.get_interfaces());
        all.extend(self.get_traits());
        all.extend(self.get_functions());
        all.extend(self.get_constants());
        all
    }

    /// Look up a symbol by its original fully-qualified name across all type maps.
    ///
    /// Returns the first match found. Search order: namespaces, classes, interfaces,
    /// traits, functions, constants.
    pub fn get_symbol_by_original(&self, original: &str) -> Option<DiscoveredSymbol> {
        let maps: [&DashMap<String, DiscoveredSymbol>; 6] = [
            &self.namespaces,
            &self.classes,
            &self.interfaces,
            &self.traits,
            &self.functions,
            &self.constants,
        ];

        for map in maps {
            if let Some(entry) = map.get(original) {
                return Some(entry.value().clone());
            }
        }

        None
    }

    /// Look up a namespace symbol by its string name.
    ///
    /// Mirrors the PHP `getNamespaceSymbolByString()` method.
    pub fn get_namespace_by_name(&self, namespace: &str) -> Option<DiscoveredSymbol> {
        self.namespaces.get(namespace).map(|e| e.value().clone())
    }

    /// Get discovered namespaces sorted by name length (shortest first),
    /// excluding the root namespace `\`.
    ///
    /// Mirrors the PHP `getDiscoveredNamespaces()` method.
    pub fn get_discovered_namespaces(&self) -> Vec<DiscoveredSymbol> {
        let mut namespaces: Vec<DiscoveredSymbol> = self
            .namespaces
            .iter()
            .filter(|entry| entry.key() != "\\")
            .map(|entry| entry.value().clone())
            .collect();

        namespaces.sort_by_key(|ns| ns.original_symbol.len());
        namespaces
    }

    /// Get global classes that should be renamed.
    ///
    /// Mirrors the PHP `getGlobalClassChanges()` method.
    pub fn get_global_class_changes(&self) -> Vec<DiscoveredSymbol> {
        self.classes
            .iter()
            .filter(|entry| entry.value().is_global_namespace() && entry.value().do_rename)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get namespace symbols that should be renamed.
    ///
    /// Mirrors the PHP `getDiscoveredNamespaceChanges()` method.
    pub fn get_namespace_changes(&self) -> Vec<DiscoveredSymbol> {
        self.get_discovered_namespaces()
            .into_iter()
            .filter(|ns| ns.do_rename)
            .collect()
    }

    /// Get function symbols that should be renamed.
    ///
    /// Mirrors the PHP `getDiscoveredFunctionChanges()` method.
    pub fn get_function_changes(&self) -> Vec<DiscoveredSymbol> {
        self.functions
            .iter()
            .filter(|entry| entry.value().do_rename)
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Get all autoloadable symbols (classes, interfaces, traits).
    ///
    /// Mirrors the PHP `getClassmapSymbols()` method.
    pub fn get_classmap_symbols(&self) -> Vec<DiscoveredSymbol> {
        let mut result = Vec::new();
        result.extend(self.get_classes());
        result.extend(self.get_interfaces());
        result.extend(self.get_traits());
        result
    }

    /// Total number of symbols across all types.
    pub fn len(&self) -> usize {
        self.namespaces.len()
            + self.classes.len()
            + self.interfaces.len()
            + self.traits.len()
            + self.functions.len()
            + self.constants.len()
    }

    /// Whether the collection is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Alias for get_classes() - get global classes only.
    pub fn get_global_classes(&self) -> Vec<DiscoveredSymbol> {
        self.get_classes()
    }

    /// Get mutable access to all symbols. Collects into a Vec for mutation.
    /// After mutation, call `update_symbols` to write back.
    pub fn all_symbols_mut(&self) -> Vec<DiscoveredSymbol> {
        self.get_all_symbols()
    }

    /// Update symbols back into the collection after mutation.
    pub fn update_symbols(&self, symbols: Vec<DiscoveredSymbol>) {
        for symbol in symbols {
            let map = self.map_for_type(&symbol.symbol_type);
            map.insert(symbol.original_symbol.clone(), symbol);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_namespace(name: &str) -> DiscoveredSymbol {
        DiscoveredSymbol::new_namespace(name.to_string(), "/test.php".to_string(), None)
    }

    fn make_class(name: &str, namespace: Option<&str>) -> DiscoveredSymbol {
        DiscoveredSymbol::new_class(
            name.to_string(),
            "/test.php".to_string(),
            namespace.map(|s| s.to_string()),
            None,
            false,
            None,
            Vec::new(),
        )
    }

    #[test]
    fn test_add_and_retrieve() {
        let symbols = DiscoveredSymbols::new();
        symbols.add(make_namespace("Vendor\\Package"));
        symbols.add(make_class("MyClass", Some("\\")));

        assert_eq!(symbols.get_namespaces().len(), 1);
        assert_eq!(symbols.get_classes().len(), 1);
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_global_vs_namespaced_classes() {
        let symbols = DiscoveredSymbols::new();
        symbols.add(make_class("GlobalClass", Some("\\")));
        symbols.add(make_class("Vendor\\NamespacedClass", Some("Vendor")));

        // get_classes returns only global namespace classes
        assert_eq!(symbols.get_classes().len(), 1);
        assert_eq!(symbols.get_classes()[0].original_symbol, "GlobalClass");

        // get_all_classes returns everything
        assert_eq!(symbols.get_all_classes().len(), 2);
    }

    #[test]
    fn test_get_symbol_by_original() {
        let symbols = DiscoveredSymbols::new();
        symbols.add(make_namespace("Vendor\\Package"));
        symbols.add(make_class("MyClass", Some("\\")));

        let found = symbols.get_symbol_by_original("Vendor\\Package");
        assert!(found.is_some());
        assert_eq!(found.unwrap().symbol_type, SymbolType::Namespace);

        let found = symbols.get_symbol_by_original("MyClass");
        assert!(found.is_some());
        assert_eq!(found.unwrap().symbol_type, SymbolType::Class);

        let not_found = symbols.get_symbol_by_original("DoesNotExist");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_merge_source_files_on_duplicate() {
        let symbols = DiscoveredSymbols::new();

        let sym1 = DiscoveredSymbol::new_class(
            "MyClass".to_string(),
            "/file1.php".to_string(),
            Some("\\".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );
        let sym2 = DiscoveredSymbol::new_class(
            "MyClass".to_string(),
            "/file2.php".to_string(),
            Some("\\".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );

        symbols.add(sym1);
        symbols.add(sym2);

        // Should still be one class, but with two source files
        assert_eq!(symbols.get_all_classes().len(), 1);
        let class = symbols.get_symbol_by_original("MyClass").unwrap();
        assert_eq!(class.source_files.len(), 2);
        assert!(class.source_files.contains(&"/file1.php".to_string()));
        assert!(class.source_files.contains(&"/file2.php".to_string()));
    }

    #[test]
    fn test_discovered_namespaces_excludes_root() {
        let symbols = DiscoveredSymbols::new();
        symbols.add(make_namespace("\\"));
        symbols.add(make_namespace("Vendor\\Package"));
        symbols.add(make_namespace("Vendor"));

        let discovered = symbols.get_discovered_namespaces();
        assert_eq!(discovered.len(), 2);
        // Sorted by length
        assert_eq!(discovered[0].original_symbol, "Vendor");
        assert_eq!(discovered[1].original_symbol, "Vendor\\Package");
    }

    #[test]
    fn test_is_empty() {
        let symbols = DiscoveredSymbols::new();
        assert!(symbols.is_empty());

        symbols.add(make_class("Foo", Some("\\")));
        assert!(!symbols.is_empty());
    }

    #[test]
    fn test_all_symbol_types() {
        let symbols = DiscoveredSymbols::new();

        symbols.add(make_namespace("NS"));
        symbols.add(make_class("Cls", Some("\\")));
        symbols.add(DiscoveredSymbol::new_interface(
            "Iface".to_string(),
            "/t.php".to_string(),
            Some("\\".to_string()),
            None,
            Vec::new(),
        ));
        symbols.add(DiscoveredSymbol::new_trait(
            "Tr".to_string(),
            "/t.php".to_string(),
            Some("\\".to_string()),
            None,
            Vec::new(),
        ));
        symbols.add(DiscoveredSymbol::new_function(
            "func".to_string(),
            "/t.php".to_string(),
            None,
            None,
        ));
        symbols.add(DiscoveredSymbol::new_constant(
            "CONST_VAL".to_string(),
            "/t.php".to_string(),
            None,
            None,
        ));

        assert_eq!(symbols.len(), 6);
        assert_eq!(symbols.get_all_symbols().len(), 6);
        assert_eq!(symbols.get_interfaces().len(), 1);
        assert_eq!(symbols.get_traits().len(), 1);
        assert_eq!(symbols.get_functions().len(), 1);
        assert_eq!(symbols.get_constants().len(), 1);
    }
}
