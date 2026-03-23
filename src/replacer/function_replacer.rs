use anyhow::Result;
use log::debug;

use crate::types::symbol::DiscoveredSymbol;

/// Functions that take a callable as their first argument
const FUNCTIONS_USING_CALLABLE: &[&str] = &[
    "function_exists",
    "call_user_func",
    "call_user_func_array",
    "forward_static_call",
    "forward_static_call_array",
    "register_shutdown_function",
    "register_tick_function",
    "unregister_tick_function",
];

/// Handles function replacement in PHP files.
/// Port of PHP Prefixer::replaceFunctions() and related methods.
///
/// Uses tree-sitter for AST-based position replacement instead of
/// nikic/php-parser.
pub struct FunctionReplacer {
    // tree-sitter parser is NOT stored here because Parser is not Send.
    // Instead, we create it per-call or use thread_local.
}

impl FunctionReplacer {
    pub fn new() -> Self {
        Self {}
    }

    /// Replace function names in PHP content using positional replacement.
    ///
    /// Port of PHP Prefixer::replaceFunctions() (lines 498-574)
    ///
    /// Finds function definitions and calls via tree-sitter AST,
    /// then replaces at exact positions (working backwards to preserve offsets).
    pub fn replace_function(
        &self,
        contents: &str,
        original_function: &str,
        replacement_function: &str,
    ) -> Result<String> {
        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_php::LANGUAGE_PHP;
        parser.set_language(&language.into())
            .map_err(|e| anyhow::anyhow!("Failed to set PHP language: {}", e))?;

        let tree = parser
            .parse(contents, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse PHP for function replacement"))?;

        let root = tree.root_node();
        let source = contents.as_bytes();
        let mut positions: Vec<(usize, usize)> = Vec::new();

        // Walk the AST to find function definitions and calls
        self.find_function_positions(
            root,
            source,
            original_function,
            &mut positions,
        );

        if positions.is_empty() {
            return Ok(contents.to_string());
        }

        // Sort positions from end to start (so replacements don't shift offsets)
        positions.sort_by(|a, b| b.0.cmp(&a.0));
        positions.dedup();

        let mut result = contents.to_string();
        for (start, end) in &positions {
            result.replace_range(*start..*end, replacement_function);
        }

        Ok(result)
    }

    /// Recursively find positions of function definitions and calls.
    fn find_function_positions(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        original_function: &str,
        positions: &mut Vec<(usize, usize)>,
    ) {
        let kind = node.kind();

        match kind {
            // Global function definition: function foo() { ... }
            "function_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = &source[name_node.start_byte()..name_node.end_byte()];
                    if let Ok(name_str) = std::str::from_utf8(name) {
                        if name_str == original_function {
                            positions.push((name_node.start_byte(), name_node.end_byte()));
                        }
                    }
                }
            }
            // Function call: foo() or \foo() or function_exists('foo')
            "function_call_expression" => {
                if let Some(name_node) = node.child_by_field_name("function") {
                    let name = &source[name_node.start_byte()..name_node.end_byte()];
                    if let Ok(name_str) = std::str::from_utf8(name) {
                        if name_str == original_function {
                            positions.push((name_node.start_byte(), name_node.end_byte()));
                        }

                        // Check if this is a function_exists('original_function') etc.
                        if FUNCTIONS_USING_CALLABLE.contains(&name_str) {
                            self.check_callable_argument(
                                node,
                                source,
                                original_function,
                                positions,
                            );
                        }
                    }
                }
            }
            _ => {}
        }

        // Recurse into children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_function_positions(child, source, original_function, positions);
        }
    }

    /// Check if a function call has a string argument matching the function name.
    /// e.g., function_exists('my_function') → replace the string content
    fn check_callable_argument(
        &self,
        call_node: tree_sitter::Node,
        source: &[u8],
        original_function: &str,
        positions: &mut Vec<(usize, usize)>,
    ) {
        if let Some(args_node) = call_node.child_by_field_name("arguments") {
            let mut cursor = args_node.walk();
            for arg in args_node.children(&mut cursor) {
                if arg.kind() == "argument" || arg.kind() == "string" || arg.kind() == "encapsed_string" {
                    // Look for the string node
                    let string_node = if arg.kind() == "string" || arg.kind() == "encapsed_string" {
                        Some(arg)
                    } else {
                        // argument > string: find child string node
                        let mut found = None;
                        let mut inner_cursor = arg.walk();
                        for child in arg.children(&mut inner_cursor) {
                            if child.kind() == "string" || child.kind() == "encapsed_string" {
                                found = Some(child);
                                break;
                            }
                        }
                        found
                    };

                    if let Some(str_node) = string_node {
                        // Get the string content (without quotes)
                        let full = &source[str_node.start_byte()..str_node.end_byte()];
                        if let Ok(full_str) = std::str::from_utf8(full) {
                            // Remove surrounding quotes
                            let inner = full_str.trim_matches('\'').trim_matches('"');
                            if inner == original_function {
                                // Replace just the inner content (start+1 to end-1 to skip quotes)
                                positions.push((
                                    str_node.start_byte() + 1,
                                    str_node.end_byte() - 1,
                                ));
                            }
                        }
                    }
                    break; // Only check first argument
                }
            }
        }
    }

    /// Prepare relative namespaces by making them absolute.
    ///
    /// Port of PHP Prefixer::prepareRelativeNamespaces() (lines 598-795)
    ///
    /// When a file uses `use Namespaced\Traitname;` without a leading backslash,
    /// and the namespace will be moved up a level, this becomes a problem.
    /// This method scans for relative namespace references and makes them absolute.
    pub fn prepare_relative_namespaces(
        &self,
        contents: &str,
        namespace_symbols: &[DiscoveredSymbol],
    ) -> Result<String> {
        if namespace_symbols.is_empty() {
            return Ok(contents.to_string());
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_php::LANGUAGE_PHP;
        parser.set_language(&language.into())
            .map_err(|e| anyhow::anyhow!("Failed to set PHP language: {}", e))?;

        let tree = parser
            .parse(contents, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse PHP for relative namespace preparation"))?;

        let root = tree.root_node();
        let source = contents.as_bytes();

        let discovered_ns: Vec<&str> = namespace_symbols
            .iter()
            .map(|s| s.original_symbol.as_str())
            .collect();

        let mut positions: Vec<(usize, usize, String)> = Vec::new();
        let mut using: Vec<String> = Vec::new();

        // Walk the AST to find relative namespace references
        self.find_relative_namespaces(
            root,
            source,
            &discovered_ns,
            &mut using,
            &mut positions,
        );

        if positions.is_empty() {
            return Ok(contents.to_string());
        }

        // Sort positions from end to start
        positions.sort_by(|a, b| b.0.cmp(&a.0));

        let mut result = contents.to_string();
        for (start, end, replacement) in &positions {
            result.replace_range(*start..*end, replacement);
        }

        // Fix any artifacts
        result = result.replace("namespace \\", "namespace ");
        result = result.replace("use \\\\", "use \\");

        Ok(result)
    }

    /// Recursively find relative namespace references in the AST.
    fn find_relative_namespaces(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        discovered_namespaces: &[&str],
        using: &mut Vec<String>,
        positions: &mut Vec<(usize, usize, String)>,
    ) {
        let kind = node.kind();

        match kind {
            "namespace_definition" => {
                if let Some(name_node) = node.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    using.push(name);
                }
            }
            "namespace_use_declaration" => {
                // use statements
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "namespace_use_clause" {
                        if let Some(name_node) = child.child_by_field_name("name") {
                            let name = node_text(name_node, source).trim_start_matches('\\').to_string();
                            using.push(name);
                        }
                    }
                }
            }
            "qualified_name" | "name" => {
                let name = node_text(node, source);
                // If the name contains \ but doesn't start with \, it may be relative
                if name.contains('\\') && !name.starts_with('\\') {
                    let parts: Vec<&str> = name.split('\\').collect();
                    if parts.len() >= 2 {
                        let ns_part: String = parts[..parts.len() - 1].join("\\");

                        // Check if the namespace part is in discovered namespaces
                        if discovered_namespaces.contains(&ns_part.as_str()) {
                            let absolute = format!("\\{}", name);
                            positions.push((node.start_byte(), node.end_byte(), absolute));
                        } else {
                            // Check with using context
                            for base in using.iter() {
                                let full = format!("{}\\{}", base, ns_part);
                                if discovered_namespaces.contains(&full.as_str()) {
                                    let absolute = format!("\\{}\\{}", base, name);
                                    positions.push((node.start_byte(), node.end_byte(), absolute));
                                    break;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        // Recurse
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_relative_namespaces(child, source, discovered_namespaces, using, positions);
        }
    }

    /// Replace fully qualified constant references with their prefixed namespaces.
    ///
    /// Port of PHP Prefixer::replaceConstFetchNamespaces() (lines 206-250)
    pub fn replace_const_fetch_namespaces(
        &self,
        contents: &str,
        namespace_symbols: &[DiscoveredSymbol],
    ) -> Result<String> {
        let rename_map: Vec<(&str, &str)> = namespace_symbols
            .iter()
            .filter(|s| s.do_rename)
            .map(|s| (s.original_symbol.as_str(), s.get_replacement()))
            .collect();

        if rename_map.is_empty() {
            return Ok(contents.to_string());
        }

        let mut parser = tree_sitter::Parser::new();
        let language = tree_sitter_php::LANGUAGE_PHP;
        parser.set_language(&language.into())
            .map_err(|e| anyhow::anyhow!("Failed to set PHP language: {}", e))?;

        let tree = parser
            .parse(contents, None)
            .ok_or_else(|| anyhow::anyhow!("Failed to parse PHP for const fetch replacement"))?;

        let root = tree.root_node();
        let source = contents.as_bytes();
        let mut positions: Vec<(usize, usize, String)> = Vec::new();

        self.find_const_fetch_positions(root, source, &rename_map, &mut positions);

        if positions.is_empty() {
            return Ok(contents.to_string());
        }

        positions.sort_by(|a, b| b.0.cmp(&a.0));

        let mut result = contents.to_string();
        for (start, end, replacement) in &positions {
            result.replace_range(*start..*end, replacement);
        }

        Ok(result)
    }

    /// Find fully qualified constant references and their replacement positions.
    fn find_const_fetch_positions(
        &self,
        node: tree_sitter::Node,
        source: &[u8],
        rename_map: &[(&str, &str)],
        positions: &mut Vec<(usize, usize, String)>,
    ) {
        // Look for qualified_name nodes that are constant references
        // In tree-sitter PHP, a fully qualified constant like \Namespace\CONST
        // appears as a qualified_name with a leading \
        if node.kind() == "qualified_name" || node.kind() == "name" {
            let text = node_text(node, source);
            if text.starts_with('\\') {
                let without_slash = &text[1..];
                let parts: Vec<&str> = without_slash.split('\\').collect();
                if let Some(first_part) = parts.first() {
                    for (original, replacement) in rename_map {
                        if *first_part == *original {
                            let mut new_parts = vec![*replacement];
                            new_parts.extend_from_slice(&parts[1..]);
                            let new_name = format!("\\{}", new_parts.join("\\"));
                            positions.push((node.start_byte(), node.end_byte(), new_name));
                            break;
                        }
                    }
                }
            }
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.find_const_fetch_positions(child, source, rename_map, positions);
        }
    }
}

/// Extract text from a tree-sitter node
fn node_text(node: tree_sitter::Node, source: &[u8]) -> String {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()])
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replace_function_definition() {
        let replacer = FunctionReplacer::new();
        let contents = r#"<?php
function my_helper() {
    return true;
}
"#;
        let result = replacer
            .replace_function(contents, "my_helper", "prefix_my_helper")
            .unwrap();

        assert!(result.contains("function prefix_my_helper()"));
    }

    #[test]
    fn test_replace_function_call() {
        let replacer = FunctionReplacer::new();
        let contents = r#"<?php
$result = my_helper();
"#;
        let result = replacer
            .replace_function(contents, "my_helper", "prefix_my_helper")
            .unwrap();

        assert!(result.contains("prefix_my_helper()"));
    }

    #[test]
    fn test_replace_function_exists() {
        let replacer = FunctionReplacer::new();
        let contents = r#"<?php
if (function_exists('my_helper')) {
    my_helper();
}
"#;
        let result = replacer
            .replace_function(contents, "my_helper", "prefix_my_helper")
            .unwrap();

        assert!(result.contains("function_exists('prefix_my_helper')"));
        assert!(result.contains("prefix_my_helper()"));
    }
}
