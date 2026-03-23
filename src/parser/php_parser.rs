use anyhow::{Context, Result};
use tree_sitter::{Parser, Tree};

/// A reusable wrapper around tree-sitter's PHP parser.
///
/// Create one instance and call `parse` repeatedly for multiple files.
pub struct PhpParser {
    parser: Parser,
}

impl PhpParser {
    /// Create a new PHP parser instance.
    pub fn new() -> Result<Self> {
        let mut parser = Parser::new();
        let language = tree_sitter_php::LANGUAGE_PHP;
        parser
            .set_language(&language.into())
            .context("Failed to set PHP language for tree-sitter parser")?;
        Ok(Self { parser })
    }

    /// Parse PHP source code into a tree-sitter syntax tree.
    ///
    /// Returns an error if parsing fails (e.g., due to a timeout or cancellation).
    pub fn parse(&mut self, source: &str) -> Result<Tree> {
        self.parser
            .parse(source, None)
            .context("tree-sitter failed to parse PHP source code")
    }
}

// PhpParser is Send but not Sync because tree_sitter::Parser uses internal
// mutable state during parsing. This is fine for our use case: one parser
// per thread.
unsafe impl Send for PhpParser {}
