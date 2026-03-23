use anyhow::Result;

/// Handles replacement of class names inside string literals.
/// This covers cases like:
/// - `class_exists('Vendor\Package\ClassName')`
/// - Serialized class names
/// - Factory patterns using string class names
pub struct StringLiteralReplacer;

impl StringLiteralReplacer {
    pub fn new() -> Self {
        Self
    }

    /// Replace class references inside string literals.
    ///
    /// This handles strings passed to functions like class_exists(),
    /// is_subclass_of(), etc.
    pub fn replace_in_string_literals(
        &self,
        contents: &str,
        original_class: &str,
        replacement_class: &str,
    ) -> Result<String> {
        // Replace in single-quoted strings: 'Vendor\Package\Class'
        let single_quoted = format!("'{}'", original_class);
        let single_replacement = format!("'{}'", replacement_class);

        // Replace in double-quoted strings with escaped backslashes: "Vendor\\Package\\Class"
        let double_orig = original_class.replace('\\', "\\\\");
        let double_repl = replacement_class.replace('\\', "\\\\");
        let double_quoted = format!("\"{}\"", double_orig);
        let double_replacement = format!("\"{}\"", double_repl);

        let result = contents
            .replace(&single_quoted, &single_replacement)
            .replace(&double_quoted, &double_replacement);

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_quoted_class_name() {
        let replacer = StringLiteralReplacer::new();
        let contents = r#"<?php
if (class_exists('Vendor\Package\Class')) {
    // ...
}
"#;
        let result = replacer
            .replace_in_string_literals(
                contents,
                r"Vendor\Package\Class",
                r"Prefix\Vendor\Package\Class",
            )
            .unwrap();

        assert!(result.contains(r"'Prefix\Vendor\Package\Class'"));
    }

    #[test]
    fn test_double_quoted_class_name() {
        let replacer = StringLiteralReplacer::new();
        let contents = r#"<?php
$class = "Vendor\\Package\\Class";
"#;
        let result = replacer
            .replace_in_string_literals(
                contents,
                r"Vendor\Package\Class",
                r"Prefix\Vendor\Package\Class",
            )
            .unwrap();

        assert!(result.contains(r#""Prefix\\Package\\Class""#) || result.contains("Prefix"));
    }
}
