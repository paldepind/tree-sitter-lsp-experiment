//! Python language implementation.

use crate::language::Language;
use tree_sitter::Node;

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

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call"]
    }

    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if !self.call_node_kinds().contains(&node.kind()) {
            return None;
        }
        // For Python, return the call node itself as goto definition target
        Some(node)
    }

    fn find_function_declaration<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Not implemented for Python
        None
    }

    fn call_hierarchy_target<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Not implemented for Python
        None
    }
}

impl std::fmt::Display for PythonLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
