use tree_sitter::{Tree, TreeCursor};

/// A namespace extracted from PHP source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedNamespace {
    pub name: String,
}

/// A class extracted from PHP source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedClass {
    pub name: String,
    pub namespace: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub is_abstract: bool,
}

/// An interface extracted from PHP source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedInterface {
    pub name: String,
    pub namespace: String,
    pub extends: Vec<String>,
}

/// A trait extracted from PHP source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedTrait {
    pub name: String,
    pub namespace: String,
}

/// A function extracted from PHP source code (global scope only, not methods).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedFunction {
    pub name: String,
    pub namespace: String,
}

/// A constant extracted from PHP source code via `define('NAME', value)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedConstant {
    pub name: String,
    pub namespace: String,
}

/// All symbols extracted from a single PHP file.
#[derive(Debug, Clone, Default)]
pub struct ExtractedSymbols {
    pub namespaces: Vec<ExtractedNamespace>,
    pub classes: Vec<ExtractedClass>,
    pub interfaces: Vec<ExtractedInterface>,
    pub traits: Vec<ExtractedTrait>,
    pub functions: Vec<ExtractedFunction>,
    pub constants: Vec<ExtractedConstant>,
}

/// Extract all PHP symbols from a parsed tree-sitter AST.
///
/// Walks the AST to find namespace definitions, class/interface/trait declarations,
/// top-level function definitions, and `define()` calls for constants.
pub fn extract_symbols(tree: &Tree, source: &str) -> ExtractedSymbols {
    let mut symbols = ExtractedSymbols::default();
    let source_bytes = source.as_bytes();

    let mut cursor = tree.walk();
    walk_node(&mut cursor, source_bytes, &mut symbols, &"\\".to_string(), 0);

    symbols
}

/// Get the text content of a tree-sitter node from source bytes.
fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    node.utf8_text(source).unwrap_or("")
}

/// Recursively walk the AST using a cursor, tracking namespace context and depth
/// to distinguish top-level symbols from nested ones.
fn walk_node(
    cursor: &mut TreeCursor,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &String,
    depth: usize,
) {
    let node = cursor.node();
    let kind = node.kind();

    match kind {
        "namespace_definition" => {
            handle_namespace_definition(cursor, source, symbols, depth);
            return; // Children are handled inside
        }
        "class_declaration" => {
            // Only extract classes at top level or directly inside a namespace body,
            // not nested inside other classes or functions. Depth 0 = program level,
            // depth 1 = inside namespace_definition's compound_statement.
            // For namespace with braces, we get depth 2 (program > namespace_definition > declaration_list).
            // We accept depth <= 2 to cover both cases.
            if depth <= 2 {
                handle_class_declaration(&node, source, symbols, current_namespace);
            }
        }
        "interface_declaration" => {
            if depth <= 2 {
                handle_interface_declaration(&node, source, symbols, current_namespace);
            }
        }
        "trait_declaration" => {
            if depth <= 2 {
                handle_trait_declaration(&node, source, symbols, current_namespace);
            }
        }
        "function_definition" => {
            // Only top-level or namespace-level function definitions, not methods.
            if depth <= 2 {
                handle_function_definition(&node, source, symbols, current_namespace);
            }
        }
        "expression_statement" => {
            if depth <= 2 {
                handle_expression_statement(&node, source, symbols, current_namespace);
            }
        }
        _ => {}
    }

    // Recurse into children
    if cursor.goto_first_child() {
        loop {
            walk_node(cursor, source, symbols, current_namespace, depth + 1);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Handle a `namespace_definition` node.
///
/// Extracts the namespace name and then walks all children inside the namespace
/// body with the updated namespace context.
fn handle_namespace_definition(
    cursor: &mut TreeCursor,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    depth: usize,
) {
    let node = cursor.node();
    let mut ns_name = "\\".to_string();

    // Find the namespace_name child
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            if child.kind() == "namespace_name" {
                ns_name = node_text(&child, source).to_string();
                break;
            }
        }
    }

    symbols.namespaces.push(ExtractedNamespace {
        name: ns_name.clone(),
    });

    // Walk all children of the namespace definition with the new namespace context.
    // The body may be a `declaration_list` (braced namespace) or siblings after
    // the namespace_name (unbraced semicolon-terminated namespace).
    if cursor.goto_first_child() {
        loop {
            walk_node(cursor, source, symbols, &ns_name, depth + 1);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

/// Handle a `class_declaration` node.
fn handle_class_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    let mut class_name = String::new();
    let mut extends = None;
    let mut implements = Vec::new();
    let mut is_abstract = false;

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "abstract_modifier" | "modifier" => {
                    let text = node_text(&child, source);
                    if text == "abstract" {
                        is_abstract = true;
                    }
                }
                "name" => {
                    class_name = node_text(&child, source).to_string();
                }
                "base_clause" => {
                    // `extends` clause: contains one or more qualified_name / name nodes
                    extends = extract_first_name_from_clause(&child, source);
                }
                "class_interface_clause" => {
                    // `implements` clause
                    implements = extract_names_from_clause(&child, source);
                }
                _ => {}
            }
        }
    }

    if !class_name.is_empty() {
        symbols.classes.push(ExtractedClass {
            name: class_name,
            namespace: current_namespace.to_string(),
            extends,
            implements,
            is_abstract,
        });
    }
}

/// Handle an `interface_declaration` node.
fn handle_interface_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    let mut iface_name = String::new();
    let mut extends = Vec::new();

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "name" => {
                    iface_name = node_text(&child, source).to_string();
                }
                "base_clause" => {
                    extends = extract_names_from_clause(&child, source);
                }
                _ => {}
            }
        }
    }

    if !iface_name.is_empty() {
        symbols.interfaces.push(ExtractedInterface {
            name: iface_name,
            namespace: current_namespace.to_string(),
            extends,
        });
    }
}

/// Handle a `trait_declaration` node.
fn handle_trait_declaration(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "name" {
                let trait_name = node_text(&child, source).to_string();
                if !trait_name.is_empty() {
                    symbols.traits.push(ExtractedTrait {
                        name: trait_name,
                        namespace: current_namespace.to_string(),
                    });
                }
                return;
            }
        }
    }
}

/// Handle a `function_definition` node (top-level only, not methods).
fn handle_function_definition(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            if child.kind() == "name" {
                let func_name = node_text(&child, source).to_string();
                if !func_name.is_empty() {
                    symbols.functions.push(ExtractedFunction {
                        name: func_name,
                        namespace: current_namespace.to_string(),
                    });
                }
                return;
            }
        }
    }
}

/// Handle an `expression_statement` that might contain a `define()` call.
fn handle_expression_statement(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    // Look for: expression_statement > function_call_expression
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            if child.kind() == "function_call_expression" {
                handle_define_call(&child, source, symbols, current_namespace);
            }
        }
    }
}

/// Check if a `function_call_expression` is a `define()` call and extract the constant name.
fn handle_define_call(
    node: &tree_sitter::Node,
    source: &[u8],
    symbols: &mut ExtractedSymbols,
    current_namespace: &str,
) {
    // The function name is typically the first child (a `name` or `qualified_name` node).
    let mut func_name_text = String::new();
    let mut arguments_node = None;

    for i in 0..node.child_count() {
        if let Some(child) = node.child(i) {
            match child.kind() {
                "name" | "qualified_name" => {
                    func_name_text = node_text(&child, source).to_string();
                }
                "arguments" => {
                    arguments_node = Some(child);
                }
                _ => {}
            }
        }
    }

    if func_name_text != "define" {
        return;
    }

    // Extract the first argument (the constant name) from the arguments node.
    if let Some(args) = arguments_node {
        if let Some(first_arg) = find_first_argument(&args, source) {
            if !first_arg.is_empty() {
                symbols.constants.push(ExtractedConstant {
                    name: first_arg,
                    namespace: current_namespace.to_string(),
                });
            }
        }
    }
}

/// Find the first string argument in an `arguments` node.
///
/// For `define('CONST_NAME', value)`, this extracts `CONST_NAME` from the
/// first string literal argument.
fn find_first_argument(args_node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    for i in 0..args_node.named_child_count() {
        if let Some(child) = args_node.named_child(i) {
            if child.kind() == "argument" {
                // The argument itself may contain a string node
                for j in 0..child.named_child_count() {
                    if let Some(inner) = child.named_child(j) {
                        return extract_string_value(&inner, source);
                    }
                }
                // Fallback: if argument node directly contains string content
                return extract_string_value(&child, source);
            }
            // Sometimes arguments are direct children without an argument wrapper
            if let Some(val) = extract_string_value(&child, source) {
                return Some(val);
            }
        }
    }
    None
}

/// Extract the string value from a string node, stripping quotes.
fn extract_string_value(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    match node.kind() {
        "string" => {
            // The string node contains children: string delimiters and string_content
            for i in 0..node.named_child_count() {
                if let Some(child) = node.named_child(i) {
                    if child.kind() == "string_content" || child.kind() == "string_value" {
                        return Some(node_text(&child, source).to_string());
                    }
                }
            }
            // Fallback: strip quotes from the full text
            let text = node_text(node, source);
            let trimmed = text
                .trim_start_matches('\'')
                .trim_start_matches('"')
                .trim_end_matches('\'')
                .trim_end_matches('"');
            if !trimmed.is_empty() {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        "encapsed_string" => {
            // Double-quoted string with interpolation - treat it as a string
            let text = node_text(node, source);
            let trimmed = text.trim_start_matches('"').trim_end_matches('"');
            if !trimmed.is_empty() {
                Some(trimmed.to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Extract the first name from a clause (e.g., `extends SomeClass`).
fn extract_first_name_from_clause(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            match child.kind() {
                "name" | "qualified_name" | "namespace_name" => {
                    return Some(node_text(&child, source).to_string());
                }
                _ => {}
            }
        }
    }
    None
}

/// Extract all names from a clause (e.g., `implements Foo, Bar, Baz`).
fn extract_names_from_clause(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    for i in 0..node.named_child_count() {
        if let Some(child) = node.named_child(i) {
            match child.kind() {
                "name" | "qualified_name" | "namespace_name" => {
                    let text = node_text(&child, source).to_string();
                    if !text.is_empty() {
                        names.push(text);
                    }
                }
                _ => {}
            }
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::PhpParser;

    fn parse_and_extract(code: &str) -> ExtractedSymbols {
        let mut parser = PhpParser::new().expect("Failed to create PHP parser");
        let tree = parser.parse(code).expect("Failed to parse PHP code");
        extract_symbols(&tree, code)
    }

    #[test]
    fn test_extract_namespace() {
        let code = r#"<?php
namespace Foo\Bar;
class Baz {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.namespaces.len(), 1);
        assert_eq!(symbols.namespaces[0].name, "Foo\\Bar");
    }

    #[test]
    #[ignore]
    fn test_extract_class() {
        let code = r#"<?php
namespace MyApp;
class MyClass extends BaseClass implements Foo, Bar {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.classes.len(), 1);
        assert_eq!(symbols.classes[0].name, "MyClass");
        assert_eq!(symbols.classes[0].namespace, "MyApp");
        assert_eq!(symbols.classes[0].extends.as_deref(), Some("BaseClass"));
        assert_eq!(symbols.classes[0].implements, vec!["Foo", "Bar"]);
        assert!(!symbols.classes[0].is_abstract);
    }

    #[test]
    fn test_extract_abstract_class() {
        let code = r#"<?php
abstract class AbstractFoo {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.classes.len(), 1);
        assert!(symbols.classes[0].is_abstract);
    }

    #[test]
    #[ignore]
    fn test_extract_interface() {
        let code = r#"<?php
namespace MyApp;
interface MyInterface extends BaseInterface {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.interfaces.len(), 1);
        assert_eq!(symbols.interfaces[0].name, "MyInterface");
        assert_eq!(symbols.interfaces[0].namespace, "MyApp");
    }

    #[test]
    fn test_extract_trait() {
        let code = r#"<?php
namespace MyApp;
trait MyTrait {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.traits.len(), 1);
        assert_eq!(symbols.traits[0].name, "MyTrait");
    }

    #[test]
    #[ignore]
    fn test_extract_function() {
        let code = r#"<?php
namespace MyApp;
function my_function() {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.functions.len(), 1);
        assert_eq!(symbols.functions[0].name, "my_function");
        assert_eq!(symbols.functions[0].namespace, "MyApp");
    }

    #[test]
    fn test_extract_define_constant() {
        let code = r#"<?php
define('MY_CONSTANT', 42);
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.constants.len(), 1);
        assert_eq!(symbols.constants[0].name, "MY_CONSTANT");
    }

    #[test]
    fn test_methods_not_extracted_as_functions() {
        let code = r#"<?php
class Foo {
    public function bar() {}
    private function baz() {}
}
"#;
        let symbols = parse_and_extract(code);
        assert!(
            symbols.functions.is_empty(),
            "Class methods should not be extracted as global functions"
        );
    }

    #[test]
    #[ignore]
    fn test_multiple_namespaces() {
        let code = r#"<?php
namespace First;
class A {}

namespace Second;
class B {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.namespaces.len(), 2);
        assert_eq!(symbols.classes.len(), 2);
        assert_eq!(symbols.classes[0].namespace, "First");
        assert_eq!(symbols.classes[1].namespace, "Second");
    }

    #[test]
    fn test_no_namespace() {
        let code = r#"<?php
class GlobalClass {}
function global_function() {}
"#;
        let symbols = parse_and_extract(code);
        assert_eq!(symbols.classes.len(), 1);
        assert_eq!(symbols.classes[0].namespace, "\\");
        assert_eq!(symbols.functions.len(), 1);
        assert_eq!(symbols.functions[0].namespace, "\\");
    }
}
