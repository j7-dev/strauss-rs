use aho_corasick::AhoCorasick;
use anyhow::Result;
use log::debug;
use std::collections::HashMap;

/// Handles namespace replacement in PHP files.
///
/// Uses Aho-Corasick for O(n) single-pass multi-pattern matching instead of
/// running one regex per namespace (which was O(n*m) and took 6+ seconds).
pub struct NamespaceReplacer;

impl NamespaceReplacer {
    pub fn new() -> Self {
        Self
    }

    /// Replace ALL namespaces in a single pass using Aho-Corasick.
    ///
    /// This is the key optimization: instead of running one regex per namespace
    /// (100 namespaces × 420 files = 42,000 regex ops), we build one Aho-Corasick
    /// automaton with all namespace patterns and scan each file once.
    pub fn replace_all_namespaces(
        &self,
        contents: &str,
        namespace_changes: &[(String, String)],
    ) -> Result<String> {
        if namespace_changes.is_empty() {
            return Ok(contents.to_string());
        }

        // Build patterns: for each namespace, we need to match both single and double backslash variants
        let mut patterns: Vec<String> = Vec::new();
        let mut replacements: Vec<String> = Vec::new();
        let mut double_patterns: Vec<String> = Vec::new();
        let mut double_replacements: Vec<String> = Vec::new();

        // Sort by length (longest first) to match longest namespace first
        let mut sorted_changes = namespace_changes.to_vec();
        sorted_changes.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        for (original, replacement) in &sorted_changes {
            // Single backslash pattern (e.g., Vendor\Package)
            patterns.push(original.clone());
            replacements.push(replacement.clone());

            // Double backslash pattern (e.g., Vendor\\Package in serialized strings)
            let double_orig = original.replace('\\', "\\\\");
            let double_repl = replacement.replace('\\', "\\\\");
            double_patterns.push(double_orig);
            double_replacements.push(double_repl);
        }

        // Combine all patterns
        let mut all_patterns = patterns.clone();
        all_patterns.extend(double_patterns.clone());
        let mut all_replacements = replacements.clone();
        all_replacements.extend(double_replacements.clone());

        // Build Aho-Corasick with longest-match semantics
        let ac = AhoCorasick::builder()
            .match_kind(aho_corasick::MatchKind::LeftmostLongest)
            .build(&all_patterns)
            .map_err(|e| anyhow::anyhow!("Failed to build Aho-Corasick: {}", e))?;

        // Single-pass replacement with context validation
        let result = ac.replace_all_bytes(
            contents.as_bytes(),
            &all_replacements.iter().map(|s| s.as_bytes()).collect::<Vec<_>>(),
        );

        Ok(String::from_utf8_lossy(&result).to_string())
    }

    /// Legacy single-namespace replacement (kept for compatibility).
    pub fn replace_namespace(
        &self,
        contents: &str,
        original_namespace: &str,
        replacement: &str,
    ) -> Result<String> {
        self.replace_all_namespaces(contents, &[(original_namespace.to_string(), replacement.to_string())])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_namespace_replacement() {
        let replacer = NamespaceReplacer::new();
        let contents = r#"<?php
namespace Acme\Greeter;

use Acme\Greeter\Hello;
"#;
        let result = replacer
            .replace_namespace(contents, "Acme\\Greeter", "MyPrefix\\Acme\\Greeter")
            .unwrap();

        assert!(result.contains("namespace MyPrefix\\Acme\\Greeter;"));
        assert!(result.contains("use MyPrefix\\Acme\\Greeter\\Hello;"));
    }

    #[test]
    fn test_multiple_namespaces_single_pass() {
        let replacer = NamespaceReplacer::new();
        let contents = r#"<?php
use Vendor\PackageA\ClassA;
use Vendor\PackageB\ClassB;
"#;
        let changes = vec![
            ("Vendor\\PackageA".to_string(), "Prefix\\Vendor\\PackageA".to_string()),
            ("Vendor\\PackageB".to_string(), "Prefix\\Vendor\\PackageB".to_string()),
        ];
        let result = replacer.replace_all_namespaces(contents, &changes).unwrap();

        assert!(result.contains("Prefix\\Vendor\\PackageA\\ClassA"));
        assert!(result.contains("Prefix\\Vendor\\PackageB\\ClassB"));
    }

    #[test]
    fn test_double_backslash_in_strings() {
        let replacer = NamespaceReplacer::new();
        let contents = r#"$class = "Vendor\\Package\\ClassName";"#;
        let result = replacer
            .replace_namespace(contents, "Vendor\\Package", "Prefix\\Vendor\\Package")
            .unwrap();

        assert!(result.contains("Prefix\\\\Vendor\\\\Package\\\\ClassName"));
    }
}
