//! TypeScript language implementation.

use crate::language::Language;
use tree_sitter::Node;

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

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "new_expression"]
    }

    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if !self.call_node_kinds().contains(&node.kind()) {
            return None;
        }
        // For TypeScript, return the call node itself as goto definition target
        Some(node)
    }

    fn find_function_declaration<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Not implemented for TypeScript
        None
    }
}

impl std::fmt::Display for TypeScriptLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
