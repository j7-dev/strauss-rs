use aho_corasick::AhoCorasick;
use anyhow::Result;
use log::debug;

/// Handles constant replacement in PHP files.
/// Uses Aho-Corasick for efficient multi-pattern string matching.
///
/// This is a massive improvement over PHP's sequential str_replace loop:
/// PHP: foreach ($constants as $c) { str_replace($c, $prefix.$c, $contents); }
/// Rust: Single-pass Aho-Corasick replaces ALL constants at once.
pub struct ConstantReplacer;

impl ConstantReplacer {
    pub fn new() -> Self {
        Self
    }

    /// Replace all constants in one pass using Aho-Corasick.
    ///
    /// Port of PHP Prefixer::replaceConstants() (lines 483-496)
    /// but dramatically more efficient: O(n) in file length regardless of constant count.
    pub fn replace_constants(
        &self,
        contents: &str,
        original_constants: &[String],
        prefix: &str,
    ) -> Result<String> {
        if original_constants.is_empty() {
            return Ok(contents.to_string());
        }

        // Build Aho-Corasick automaton from all constant names
        let ac = AhoCorasick::new(original_constants)
            .map_err(|e| anyhow::anyhow!("Failed to build Aho-Corasick for constants: {}", e))?;

        // Build replacement strings: prefix + original
        let replacements: Vec<String> = original_constants
            .iter()
            .map(|c| format!("{}{}", prefix, c))
            .collect();

        // Single-pass replacement
        let replacement_refs: Vec<&str> = replacements.iter().map(|s| s.as_str()).collect();
        let result = ac.replace_all(contents, &replacement_refs);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_constant_replacement() {
        let replacer = ConstantReplacer::new();
        let contents = r#"<?php
define('FPDF_VERSION', '1.82');
echo FPDF_VERSION;
"#;
        let constants = vec!["FPDF_VERSION".to_string()];
        let result = replacer
            .replace_constants(contents, &constants, "MY_PREFIX_")
            .unwrap();

        assert!(result.contains("MY_PREFIX_FPDF_VERSION"));
        assert!(!result.contains("define('FPDF_VERSION'"));
    }

    #[test]
    fn test_multiple_constants_replacement() {
        let replacer = ConstantReplacer::new();
        let contents = r#"<?php
define('CONST_A', 1);
define('CONST_B', 2);
$x = CONST_A + CONST_B;
"#;
        let constants = vec!["CONST_A".to_string(), "CONST_B".to_string()];
        let result = replacer
            .replace_constants(contents, &constants, "PFX_")
            .unwrap();

        assert!(result.contains("PFX_CONST_A"));
        assert!(result.contains("PFX_CONST_B"));
    }

    #[test]
    fn test_empty_constants() {
        let replacer = ConstantReplacer::new();
        let contents = "<?php\necho 'hello';\n";
        let result = replacer
            .replace_constants(contents, &[], "PFX_")
            .unwrap();

        assert_eq!(result, contents);
    }
}
