//! Programming language definitions and configurations.

use std::fmt::{Debug, Display};

use anyhow::Result;
use regex::Regex;
use tree_sitter::Node;

/// Trait representing a programming language for Tree Sitter parsing and LSP integration
pub trait Language: Debug + Display + Copy {
    /// Returns the lowercase name used for command line arguments
    fn cli_name(&self) -> &'static str;

    /// Returns a regex pattern that matches files for this language
    fn file_pattern(&self) -> &'static str;

    /// Returns the file extensions for this language as a human-readable string
    fn extensions(&self) -> &'static str;

    /// Returns the display name for this language
    fn display_name(&self) -> &'static str;

    /// Returns the LSP server command and arguments for this language
    fn lsp_server_command(&self) -> (&'static str, Vec<String>);

    /// Returns the Tree Sitter language grammar for the given language
    fn tree_sitter_language(&self) -> tree_sitter::Language;

    /// Returns the node kinds that represent calls in this language
    fn call_node_kinds(&self) -> &'static [&'static str];

    /// Finds the appropriate node for goto definition within a call node
    /// For method calls, this returns the method name node; otherwise returns the call node itself
    /// Returns None if the node is not a call node for this language
    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>>;

    /// Creates a compiled regex for matching files of this language
    fn file_regex(&self) -> Result<Regex> {
        Regex::new(self.file_pattern())
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::languages::{GoLang, PythonLang, RustLang, TypeScriptLang};

    #[test]
    fn test_language_patterns() {
        assert_eq!(RustLang.file_pattern(), r"\.rs$");
        assert_eq!(PythonLang.file_pattern(), r"\.py$");
        assert_eq!(TypeScriptLang.file_pattern(), r"\.(ts|tsx)$");
        assert_eq!(GoLang.file_pattern(), r"\.go$");
    }

    #[test]
    fn test_file_regex() {
        let rust_regex = RustLang.file_regex().unwrap();
        assert!(rust_regex.is_match("main.rs"));
        assert!(rust_regex.is_match("lib.rs"));
        assert!(!rust_regex.is_match("main.py"));
        assert!(!rust_regex.is_match("main.rs.bak"));

        let ts_regex = TypeScriptLang.file_regex().unwrap();
        assert!(ts_regex.is_match("app.ts"));
        assert!(ts_regex.is_match("component.tsx"));
        assert!(!ts_regex.is_match("app.js"));
    }
}
