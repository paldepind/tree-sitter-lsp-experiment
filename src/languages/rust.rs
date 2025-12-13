//! Rust language implementation.

use crate::language::Language;
use tree_sitter::Node;

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

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "macro_invocation"]
    }

    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if !self.call_node_kinds().contains(&node.kind()) {
            return None;
        }
        // For Rust, return the call node itself as goto definition target
        Some(node)
    }
}

impl std::fmt::Display for RustLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
