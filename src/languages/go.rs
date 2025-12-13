//! Go language implementation.

use crate::language::Language;
use tree_sitter::Node;

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

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression"]
    }

    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if !self.call_node_kinds().contains(&node.kind()) {
            return None;
        }
        // For Go, return the call node itself as goto definition target
        Some(node)
    }
}

impl std::fmt::Display for GoLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
