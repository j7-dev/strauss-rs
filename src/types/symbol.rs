use std::fmt;

/// The type of a discovered PHP symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolType {
    Namespace,
    Class,
    Interface,
    Trait,
    Function,
    Constant,
}

impl fmt::Display for SymbolType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SymbolType::Namespace => write!(f, "namespace"),
            SymbolType::Class => write!(f, "class"),
            SymbolType::Interface => write!(f, "interface"),
            SymbolType::Trait => write!(f, "trait"),
            SymbolType::Function => write!(f, "function"),
            SymbolType::Constant => write!(f, "constant"),
        }
    }
}

/// A namespace, class, interface, trait, function, or constant discovered in the project.
///
/// This is a unified struct that covers all symbol types. Fields that are only relevant
/// to specific symbol types (e.g. `extends`, `interfaces`, `is_abstract`) are only
/// meaningful when `symbol_type` is the corresponding variant.
#[derive(Debug, Clone)]
pub struct DiscoveredSymbol {
    /// The kind of symbol (namespace, class, interface, trait, function, constant).
    pub symbol_type: SymbolType,

    /// The fully-qualified original symbol name, e.g. `Vendor\Package\ClassName`.
    pub original_symbol: String,

    /// The replacement symbol name after prefixing.
    /// When `do_rename` is false, `get_replacement()` returns `original_symbol` instead.
    pub replacement: Option<String>,

    /// The parent namespace of this symbol, if any.
    /// For the global namespace this is `None` or `Some("\\")`.
    pub namespace: Option<String>,

    /// Whether this symbol should be renamed during prefixing. Defaults to `true`.
    pub do_rename: bool,

    /// File paths where this symbol was discovered.
    pub source_files: Vec<String>,

    /// The Composer package name this symbol belongs to, if any.
    pub package_name: Option<String>,

    // --- Class-specific fields ---
    /// The class this symbol extends (only meaningful for `SymbolType::Class`).
    pub extends: Option<String>,

    /// Interfaces implemented by this class (only meaningful for `SymbolType::Class`).
    /// For `SymbolType::Interface`, this holds the interfaces it extends.
    /// For `SymbolType::Trait`, this holds the traits it uses.
    pub interfaces: Vec<String>,

    /// Whether this class is abstract (only meaningful for `SymbolType::Class`).
    pub is_abstract: bool,
}

impl DiscoveredSymbol {
    /// Create a new namespace symbol.
    pub fn new_namespace(fqdn: String, source_file: String, package_name: Option<String>) -> Self {
        Self {
            symbol_type: SymbolType::Namespace,
            original_symbol: fqdn,
            replacement: None,
            namespace: None,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        }
    }

    /// Create a new class symbol.
    pub fn new_class(
        fqdn: String,
        source_file: String,
        namespace: Option<String>,
        package_name: Option<String>,
        is_abstract: bool,
        extends: Option<String>,
        interfaces: Vec<String>,
    ) -> Self {
        Self {
            symbol_type: SymbolType::Class,
            original_symbol: fqdn,
            replacement: None,
            namespace,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends,
            interfaces,
            is_abstract,
        }
    }

    /// Create a new interface symbol.
    pub fn new_interface(
        fqdn: String,
        source_file: String,
        namespace: Option<String>,
        package_name: Option<String>,
        extends: Vec<String>,
    ) -> Self {
        Self {
            symbol_type: SymbolType::Interface,
            original_symbol: fqdn,
            replacement: None,
            namespace,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends: None,
            interfaces: extends, // Interface "extends" stored in the interfaces field
            is_abstract: false,
        }
    }

    /// Create a new trait symbol.
    pub fn new_trait(
        fqdn: String,
        source_file: String,
        namespace: Option<String>,
        package_name: Option<String>,
        uses: Vec<String>,
    ) -> Self {
        Self {
            symbol_type: SymbolType::Trait,
            original_symbol: fqdn,
            replacement: None,
            namespace,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends: None,
            interfaces: uses, // Trait "uses" stored in the interfaces field
            is_abstract: false,
        }
    }

    /// Create a new function symbol.
    pub fn new_function(
        fqdn: String,
        source_file: String,
        namespace: Option<String>,
        package_name: Option<String>,
    ) -> Self {
        Self {
            symbol_type: SymbolType::Function,
            original_symbol: fqdn,
            replacement: None,
            namespace,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        }
    }

    /// Create a new constant symbol.
    pub fn new_constant(
        fqdn: String,
        source_file: String,
        namespace: Option<String>,
        package_name: Option<String>,
    ) -> Self {
        Self {
            symbol_type: SymbolType::Constant,
            original_symbol: fqdn,
            replacement: None,
            namespace,
            do_rename: true,
            source_files: vec![source_file],
            package_name,
            extends: None,
            interfaces: Vec::new(),
            is_abstract: false,
        }
    }

    /// Get the original fully-qualified symbol name.
    pub fn get_original_symbol(&self) -> &str {
        &self.original_symbol
    }

    /// Get the effective replacement name.
    ///
    /// If `do_rename` is false, returns the original symbol name.
    /// If `do_rename` is true and a replacement has been set, returns the replacement.
    /// If `do_rename` is true but no replacement has been set, returns the original symbol name.
    pub fn get_replacement(&self) -> &str {
        if !self.do_rename {
            return &self.original_symbol;
        }
        match &self.replacement {
            Some(r) => r.as_str(),
            None => &self.original_symbol,
        }
    }

    /// Set the replacement name for this symbol.
    pub fn set_replacement(&mut self, replacement: String) {
        self.replacement = Some(replacement);
    }

    /// Get the local (unqualified) name portion of the symbol.
    ///
    /// For example, `Vendor\Package\ClassName` returns `ClassName`.
    pub fn get_original_local_name(&self) -> &str {
        self.original_symbol
            .rsplit('\\')
            .next()
            .unwrap_or(&self.original_symbol)
    }

    /// Add a source file path where this symbol was discovered.
    pub fn add_source_file(&mut self, source_file: String) {
        if !self.source_files.contains(&source_file) {
            self.source_files.push(source_file);
        }
    }

    /// Check whether the namespace has changed (replacement differs from original).
    ///
    /// Only meaningful for `SymbolType::Namespace`.
    pub fn is_changed_namespace(&self) -> bool {
        self.get_replacement() != self.original_symbol
    }

    /// Whether this symbol is in the global namespace.
    pub fn is_global_namespace(&self) -> bool {
        match &self.namespace {
            None => true,
            Some(ns) => ns == "\\" || ns.is_empty(),
        }
    }

    /// Whether this symbol is a namespace symbol.
    pub fn is_namespace(&self) -> bool {
        self.symbol_type == SymbolType::Namespace
    }
}

// Safety: DiscoveredSymbol is Send + Sync because it only contains owned types
// (String, Vec<String>, bool, Option<String>) which are all Send + Sync.
// This is automatically derived by the compiler, but we note it here for clarity.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_namespace() {
        let sym = DiscoveredSymbol::new_namespace(
            "Vendor\\Package".to_string(),
            "/path/to/file.php".to_string(),
            Some("vendor/package".to_string()),
        );
        assert_eq!(sym.symbol_type, SymbolType::Namespace);
        assert_eq!(sym.original_symbol, "Vendor\\Package");
        assert_eq!(sym.get_replacement(), "Vendor\\Package");
        assert!(sym.do_rename);
    }

    #[test]
    fn test_replacement_logic() {
        let mut sym = DiscoveredSymbol::new_class(
            "MyClass".to_string(),
            "/path/to/file.php".to_string(),
            Some("\\".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );

        // No replacement set, returns original
        assert_eq!(sym.get_replacement(), "MyClass");

        // Set replacement
        sym.set_replacement("Prefixed_MyClass".to_string());
        assert_eq!(sym.get_replacement(), "Prefixed_MyClass");

        // Disable rename
        sym.do_rename = false;
        assert_eq!(sym.get_replacement(), "MyClass");
    }

    #[test]
    fn test_local_name() {
        let sym = DiscoveredSymbol::new_class(
            "Vendor\\Package\\ClassName".to_string(),
            "/path/to/file.php".to_string(),
            Some("Vendor\\Package".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );
        assert_eq!(sym.get_original_local_name(), "ClassName");
    }

    #[test]
    fn test_local_name_no_namespace() {
        let sym = DiscoveredSymbol::new_class(
            "SimpleClass".to_string(),
            "/path/to/file.php".to_string(),
            Some("\\".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );
        assert_eq!(sym.get_original_local_name(), "SimpleClass");
    }

    #[test]
    fn test_is_global_namespace() {
        let global_sym = DiscoveredSymbol::new_class(
            "MyClass".to_string(),
            "/path/to/file.php".to_string(),
            Some("\\".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );
        assert!(global_sym.is_global_namespace());

        let namespaced_sym = DiscoveredSymbol::new_class(
            "Vendor\\MyClass".to_string(),
            "/path/to/file.php".to_string(),
            Some("Vendor".to_string()),
            None,
            false,
            None,
            Vec::new(),
        );
        assert!(!namespaced_sym.is_global_namespace());
    }

    #[test]
    fn test_add_source_file_dedup() {
        let mut sym = DiscoveredSymbol::new_function(
            "my_function".to_string(),
            "/path/to/a.php".to_string(),
            None,
            None,
        );
        sym.add_source_file("/path/to/a.php".to_string());
        sym.add_source_file("/path/to/b.php".to_string());
        assert_eq!(sym.source_files.len(), 2);
    }

    #[test]
    fn test_symbol_type_display() {
        assert_eq!(SymbolType::Namespace.to_string(), "namespace");
        assert_eq!(SymbolType::Class.to_string(), "class");
        assert_eq!(SymbolType::Interface.to_string(), "interface");
        assert_eq!(SymbolType::Trait.to_string(), "trait");
        assert_eq!(SymbolType::Function.to_string(), "function");
        assert_eq!(SymbolType::Constant.to_string(), "constant");
    }
}
