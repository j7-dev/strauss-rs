use tree_sitter::{Tree, TreeCursor};
use crate::types::file_references::FileReferences;

/// Extract all symbol references from a parsed tree-sitter AST.
///
/// Walks the AST once to collect every identifier and namespace-like string
/// that appears in the file. This data is used to filter the global symbol
/// list down to a per-file subset for efficient replacement.
///
/// This is NOT the same as symbol_extractor (which finds declarations).
/// This finds all USAGES/references.
pub fn extract_references(tree: &Tree, source: &str) -> FileReferences {
    let mut refs = FileReferences::new();
    let source_bytes = source.as_bytes();
    let mut cursor = tree.walk();
    walk_for_references(&mut cursor, source_bytes, &mut refs);
    refs
}

/// Get the text content of a tree-sitter node.
fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Recursively walk the AST, extracting references at each node.
fn walk_for_references(
    cursor: &mut TreeCursor,
    source: &[u8],
    refs: &mut FileReferences,
) {
    let node = cursor.node();
    let kind = node.kind();

    match kind {
        // Namespace declaration: namespace Foo\Bar;
        "namespace_definition" => {
            if let Some(name_node) = find_child_by_kind(&node, "namespace_name") {
                let text = node_text(&name_node, source);
                refs.add_namespace_with_prefixes(text);
            }
        }

        // Use statements: use Foo\Bar\Baz; / use function Foo\func;
        "namespace_use_declaration" => {
            extract_use_declaration(&node, source, refs);
        }

        // Qualified names anywhere: Foo\Bar\Baz or \Foo\Bar\Baz
        "qualified_name" => {
            let text = node_text(&node, source);
            let clean = text.trim_start_matches('\\');
            if clean.contains('\\') {
                refs.add_namespace_with_prefixes(clean);
            } else if !clean.is_empty() {
                refs.identifier_references.insert(clean.to_string());
            }
        }

        // Bare namespace_name (not inside qualified_name)
        "namespace_name" => {
            // Only process if parent is not qualified_name (avoid double processing)
            let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
            if parent_kind != "qualified_name" {
                let text = node_text(&node, source);
                if text.contains('\\') {
                    refs.add_namespace_with_prefixes(text);
                }
            }
        }

        // Simple name nodes in reference contexts
        "name" => {
            let parent_kind = node.parent().map(|p| p.kind()).unwrap_or("");
            match parent_kind {
                "named_type" | "base_clause" | "class_interface_clause"
                | "object_creation_expression" | "class_constant_access_expression"
                | "scoped_call_expression" | "catch_clause"
                | "function_call_expression" | "use_as_clause"
                | "namespace_use_clause" | "property_declaration"
                | "enum_declaration" | "use_declaration" => {
                    let text = node_text(&node, source);
                    if !text.is_empty() {
                        refs.identifier_references.insert(text.to_string());
                    }
                }
                _ => {}
            }
        }

        // String literals - check for namespace patterns
        "string" => {
            extract_string_references(&node, source, refs);
        }

        // Double-quoted strings with potential interpolation
        "encapsed_string" => {
            let text = node_text(&node, source);
            let inner = text.trim_start_matches('"').trim_end_matches('"');
            if inner.contains('\\') {
                refs.string_namespace_references.insert(inner.to_string());
                // Normalize double backslash to single for namespace matching
                let normalized = inner.replace("\\\\", "\\");
                if normalized.contains('\\') {
                    refs.add_namespace_with_prefixes(&normalized);
                }
            }
        }

        // Comment nodes - may contain namespace references in doc blocks
        "comment" => {
            let text = node_text(&node, source);
            // Quick check: only process if it looks like it contains a namespace
            if text.contains('\\') {
                extract_namespaces_from_text(text, refs);
            }
        }

        _ => {}
    }

    // Recurse into children
    if cursor.goto_first_child() {
        loop {
            walk_for_references(cursor, source, refs);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Extract references from a use declaration.
/// Handles: use Foo\Bar; / use Foo\Bar as Baz; / use function Foo\func;
fn extract_use_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    refs: &mut FileReferences,
) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "namespace_use_clause" | "namespace_use_group_clause" => {
                    // Find namespace_name or qualified_name inside
                    for j in 0..child.child_count() {
                        if let Some(inner) = child.child(j) {
                            match inner.kind() {
                                "namespace_name" | "qualified_name" => {
                                    let text = node_text(&inner, source);
                                    let clean = text.trim_start_matches('\\');
                                    if clean.contains('\\') {
                                        refs.add_namespace_with_prefixes(clean);
                                    } else if !clean.is_empty() {
                                        refs.identifier_references.insert(clean.to_string());
                                    }
                                }
                                "name" => {
                                    let text = node_text(&inner, source);
                                    if !text.is_empty() {
                                        refs.identifier_references.insert(text.to_string());
                                    }
                                }
                                "namespace_use_group" => {
                                    // Grouped use: use Foo\{Bar, Baz};
                                    extract_use_group(&inner, source, refs);
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "namespace_name" | "qualified_name" => {
                    let text = node_text(&child, source);
                    let clean = text.trim_start_matches('\\');
                    if clean.contains('\\') {
                        refs.add_namespace_with_prefixes(clean);
                    } else if !clean.is_empty() {
                        refs.identifier_references.insert(clean.to_string());
                    }
                }
                _ => {}
            }
        }
    }
}

/// Extract references from a grouped use clause: use Foo\{Bar, Baz};
fn extract_use_group(
    node: &tree_sitter::Node,
    source: &[u8],
    refs: &mut FileReferences,
) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "namespace_use_clause" => {
                    for j in 0..child.child_count() {
                        if let Some(inner) = child.child(j) {
                            if inner.kind() == "name" || inner.kind() == "namespace_name" {
                                let text = node_text(&inner, source);
                                if !text.is_empty() {
                                    refs.identifier_references.insert(text.to_string());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Extract string content and check for namespace-like patterns.
fn extract_string_references(
    node: &tree_sitter::Node,
    source: &[u8],
    refs: &mut FileReferences,
) {
    // Get inner content (strip quotes)
    let text = node_text(node, source);
    let inner = text.trim_start_matches('\'').trim_end_matches('\'');

    if inner.contains('\\') {
        refs.string_namespace_references.insert(inner.to_string());
        // If it looks like a namespace (contains single backslash, not escaped),
        // also add to namespace_references
        if !inner.starts_with("\\\\") {
            refs.add_namespace_with_prefixes(inner);
        }
    }
}

/// Extract namespace-like patterns from arbitrary text (e.g., doc comments).
fn extract_namespaces_from_text(text: &str, refs: &mut FileReferences) {
    // Simple heuristic: find sequences like Word\Word(\Word)*
    // This catches @param \Vendor\Class, @return Vendor\Class, etc.
    let mut i = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    while i < len {
        // Find start of a potential namespace (letter or backslash)
        if i < len && (bytes[i] == b'\\' || bytes[i].is_ascii_alphabetic() || bytes[i] == b'_') {
            let start = i;
            let mut has_backslash = false;

            // Consume word characters and backslashes
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'\\') {
                if bytes[i] == b'\\' {
                    has_backslash = true;
                }
                i += 1;
            }

            if has_backslash && i > start + 2 {
                let segment = &text[start..i];
                let clean = segment.trim_start_matches('\\').trim_end_matches('\\');
                if clean.contains('\\') && !clean.starts_with('\\') {
                    refs.add_namespace_with_prefixes(clean);
                }
            }
        } else {
            i += 1;
        }
    }
}

/// Find a direct child node of a given kind.
fn find_child_by_kind<'a>(node: &'a tree_sitter::Node, kind: &str) -> Option<tree_sitter::Node<'a>> {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == kind {
                return Some(child);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::PhpParser;

    fn parse_and_extract_refs(code: &str) -> FileReferences {
        let mut parser = PhpParser::new().expect("Failed to create parser");
        let tree = parser.parse(code).expect("Failed to parse");
        extract_references(&tree, code)
    }

    #[test]
    fn test_namespace_declaration() {
        let refs = parse_and_extract_refs("<?php\nnamespace Vendor\\Package;");
        assert!(refs.namespace_references.contains("Vendor\\Package"));
        assert!(refs.namespace_references.contains("Vendor"));
    }

    #[test]
    fn test_use_statement() {
        let refs = parse_and_extract_refs("<?php\nuse Vendor\\Package\\ClassName;");
        assert!(refs.namespace_references.contains("Vendor\\Package\\ClassName"));
        assert!(refs.namespace_references.contains("Vendor\\Package"));
        assert!(refs.namespace_references.contains("Vendor"));
    }

    #[test]
    fn test_qualified_name_in_code() {
        let refs = parse_and_extract_refs("<?php\n$x = new \\Vendor\\Package\\Foo();");
        assert!(refs.namespace_references.contains("Vendor\\Package\\Foo"));
        assert!(refs.namespace_references.contains("Vendor\\Package"));
    }

    #[test]
    fn test_type_hint() {
        let refs = parse_and_extract_refs("<?php\nfunction foo(MyClass $x) {}");
        assert!(refs.identifier_references.contains("MyClass"));
    }

    #[test]
    fn test_string_with_namespace() {
        let refs = parse_and_extract_refs("<?php\n$c = 'Vendor\\Package\\ClassName';");
        assert!(!refs.string_namespace_references.is_empty());
        assert!(refs.could_reference_namespace("Vendor\\Package"));
    }

    #[test]
    fn test_extends_implements() {
        let refs = parse_and_extract_refs(
            "<?php\nclass Foo extends BaseClass implements MyInterface {}"
        );
        assert!(refs.identifier_references.contains("BaseClass"));
        assert!(refs.identifier_references.contains("MyInterface"));
    }

    #[test]
    fn test_static_access() {
        let refs = parse_and_extract_refs("<?php\nMyClass::doSomething();");
        assert!(refs.identifier_references.contains("MyClass"));
    }

    #[test]
    fn test_function_call() {
        let refs = parse_and_extract_refs("<?php\nmy_function();");
        assert!(refs.identifier_references.contains("my_function"));
    }

    #[test]
    fn test_catch_block() {
        let refs = parse_and_extract_refs(
            "<?php\ntry {} catch (RuntimeException $e) {}"
        );
        assert!(refs.identifier_references.contains("RuntimeException"));
    }
}
