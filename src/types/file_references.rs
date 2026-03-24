use std::collections::HashSet;

/// Per-file reference data: tracks which symbols a PHP file mentions.
///
/// Populated during Phase 2 (tree-sitter parsing) at near-zero cost
/// since the AST is already in memory. Used in Phase 3 to filter the
/// global symbol list down to only the symbols relevant to each file,
/// enabling tiny per-file Aho-Corasick automata instead of one giant one.
#[derive(Debug, Clone, Default)]
pub struct FileReferences {
    /// Namespace-like strings found in the file (single backslash form).
    /// Sources: namespace_definition, namespace_use_declaration,
    /// qualified_name, namespace_name nodes.
    pub namespace_references: HashSet<String>,

    /// Simple identifiers that could be class/function/constant names.
    /// Sources: name nodes in type hint, new, extends, implements,
    /// catch, function_call, static access contexts.
    pub identifier_references: HashSet<String>,

    /// String literal contents containing backslashes (potential namespace refs).
    /// Sources: string, encapsed_string nodes containing '\'.
    pub string_namespace_references: HashSet<String>,
}

impl FileReferences {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any reference in this file could match the given namespace.
    ///
    /// Matches if:
    /// - The exact namespace appears in namespace_references
    /// - A longer namespace starting with `{namespace}\` appears (child namespace)
    /// - A string literal contains the namespace (single or double backslash)
    pub fn could_reference_namespace(&self, namespace: &str) -> bool {
        // Exact match
        if self.namespace_references.contains(namespace) {
            return true;
        }
        // Prefix match: file references A\B\C\D, namespace is A\B
        let prefix = format!("{}\\", namespace);
        if self.namespace_references.iter().any(|r| r.starts_with(&prefix)) {
            return true;
        }
        // String literal match (single or double backslash)
        let double_bs = namespace.replace('\\', "\\\\");
        self.string_namespace_references.iter().any(|s| {
            s.contains(namespace) || s.contains(&double_bs)
        })
    }

    /// Check if a simple identifier (class name, function name, constant) appears.
    pub fn could_reference_identifier(&self, name: &str) -> bool {
        self.identifier_references.contains(name)
    }

    /// Add a namespace reference and all its parent prefixes.
    /// e.g. "A\B\C\D" adds "A\B\C\D", "A\B\C", "A\B", "A"
    pub fn add_namespace_with_prefixes(&mut self, namespace: &str) {
        let clean = namespace.trim_start_matches('\\');
        if clean.is_empty() {
            return;
        }
        self.namespace_references.insert(clean.to_string());
        // Add all parent namespace prefixes
        let mut pos = clean.len();
        while let Some(idx) = clean[..pos].rfind('\\') {
            self.namespace_references.insert(clean[..idx].to_string());
            pos = idx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_namespace_match() {
        let mut refs = FileReferences::new();
        refs.namespace_references.insert("Vendor\\Package".to_string());
        assert!(refs.could_reference_namespace("Vendor\\Package"));
        assert!(!refs.could_reference_namespace("Other\\Package"));
    }

    #[test]
    fn test_prefix_namespace_match() {
        let mut refs = FileReferences::new();
        refs.namespace_references.insert("Vendor\\Package\\Sub\\Class".to_string());
        // Parent namespace should match
        assert!(refs.could_reference_namespace("Vendor\\Package"));
        assert!(refs.could_reference_namespace("Vendor\\Package\\Sub"));
        assert!(!refs.could_reference_namespace("Other"));
    }

    #[test]
    fn test_string_namespace_match() {
        let mut refs = FileReferences::new();
        refs.string_namespace_references.insert("Vendor\\\\Package\\\\ClassName".to_string());
        // Should match via double-backslash
        assert!(refs.could_reference_namespace("Vendor\\Package"));
    }

    #[test]
    fn test_identifier_match() {
        let mut refs = FileReferences::new();
        refs.identifier_references.insert("MyClass".to_string());
        assert!(refs.could_reference_identifier("MyClass"));
        assert!(!refs.could_reference_identifier("OtherClass"));
    }

    #[test]
    fn test_add_namespace_with_prefixes() {
        let mut refs = FileReferences::new();
        refs.add_namespace_with_prefixes("A\\B\\C\\D");
        assert!(refs.namespace_references.contains("A\\B\\C\\D"));
        assert!(refs.namespace_references.contains("A\\B\\C"));
        assert!(refs.namespace_references.contains("A\\B"));
        assert!(refs.namespace_references.contains("A"));
    }

    #[test]
    fn test_add_namespace_strips_leading_backslash() {
        let mut refs = FileReferences::new();
        refs.add_namespace_with_prefixes("\\Vendor\\Package");
        assert!(refs.namespace_references.contains("Vendor\\Package"));
        assert!(refs.namespace_references.contains("Vendor"));
    }
}
