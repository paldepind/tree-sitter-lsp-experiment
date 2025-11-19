//! Programming language definitions and configurations.

use anyhow::Result;
use regex::Regex;

/// Trait representing a programming language for Tree Sitter parsing and LSP integration
pub trait Language: std::fmt::Debug + std::fmt::Display + Copy {
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

    /// Creates a compiled regex for matching files of this language
    fn file_regex(&self) -> Result<Regex> {
        Regex::new(self.file_pattern())
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))
    }
}

/// Rust language implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RustLang;

impl Language for RustLang {
    fn cli_name(&self) -> &'static str {
        "rust"
    }

    fn file_pattern(&self) -> &'static str {
        r"\.rs$"
    }

    fn extensions(&self) -> &'static str {
        ".rs"
    }

    fn display_name(&self) -> &'static str {
        "Rust"
    }

    fn lsp_server_command(&self) -> (&'static str, Vec<String>) {
        ("rust-analyzer", vec![])
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_rust::LANGUAGE.into()
    }
}

impl std::fmt::Display for RustLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Python language implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PythonLang;

impl Language for PythonLang {
    fn cli_name(&self) -> &'static str {
        "python"
    }

    fn file_pattern(&self) -> &'static str {
        r"\.py$"
    }

    fn extensions(&self) -> &'static str {
        ".py"
    }

    fn display_name(&self) -> &'static str {
        "Python"
    }

    fn lsp_server_command(&self) -> (&'static str, Vec<String>) {
        ("pylsp", vec![])
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_python::LANGUAGE.into()
    }
}

impl std::fmt::Display for PythonLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// TypeScript language implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeScriptLang;

impl Language for TypeScriptLang {
    fn cli_name(&self) -> &'static str {
        "typescript"
    }

    fn file_pattern(&self) -> &'static str {
        r"\.(ts|tsx)$"
    }

    fn extensions(&self) -> &'static str {
        ".ts, .tsx"
    }

    fn display_name(&self) -> &'static str {
        "TypeScript"
    }

    fn lsp_server_command(&self) -> (&'static str, Vec<String>) {
        ("typescript-language-server", vec!["--stdio".to_string()])
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()
    }
}

impl std::fmt::Display for TypeScriptLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Go language implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GoLang;

impl Language for GoLang {
    fn cli_name(&self) -> &'static str {
        "go"
    }

    fn file_pattern(&self) -> &'static str {
        r"\.go$"
    }

    fn extensions(&self) -> &'static str {
        ".go"
    }

    fn display_name(&self) -> &'static str {
        "Go"
    }

    fn lsp_server_command(&self) -> (&'static str, Vec<String>) {
        ("gopls", vec![])
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_go::LANGUAGE.into()
    }
}

impl std::fmt::Display for GoLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Swift language implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SwiftLang;

impl Language for SwiftLang {
    fn cli_name(&self) -> &'static str {
        "swift"
    }

    fn file_pattern(&self) -> &'static str {
        r"\.swift$"
    }

    fn extensions(&self) -> &'static str {
        ".swift"
    }

    fn display_name(&self) -> &'static str {
        "Swift"
    }

    fn lsp_server_command(&self) -> (&'static str, Vec<String>) {
        ("sourcekit-lsp", vec![])
    }

    fn tree_sitter_language(&self) -> tree_sitter::Language {
        tree_sitter_swift::LANGUAGE.into()
    }
}

impl std::fmt::Display for SwiftLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
