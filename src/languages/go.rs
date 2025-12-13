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

    fn find_function_declaration<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Check if this is a function declaration
        if node.kind() != "function_declaration" && node.kind() != "method_declaration" {
            return None;
        }

        // Find the identifier child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "identifier" || child.kind() == "field_identifier")
    }
}

impl std::fmt::Display for GoLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_function_declaration() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser.set_language(&GoLang.tree_sitter_language()).unwrap();

        let source = "package main\n\nfunc hello() { println(\"Hello\") }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Manually find the function_declaration node
        let mut cursor = root.walk();
        let mut function_node = None;
        for child in root.children(&mut cursor) {
            if child.kind() == "function_declaration" {
                function_node = Some(child);
                break;
            }
        }
        let function_node = function_node.expect("Should find function_declaration");

        // Test find_function_declaration
        let identifier = GoLang.find_function_declaration(function_node);
        assert!(identifier.is_some());
        let identifier = identifier.unwrap();
        assert_eq!(identifier.kind(), "identifier");
        assert_eq!(identifier.utf8_text(source.as_bytes()).unwrap(), "hello");
    }

    #[test]
    fn test_find_method_declaration() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser.set_language(&GoLang.tree_sitter_language()).unwrap();

        let source = "package main\n\nfunc (r Receiver) Method() {}";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Manually find the method_declaration node
        let mut cursor = root.walk();
        let mut method_node = None;
        for child in root.children(&mut cursor) {
            if child.kind() == "method_declaration" {
                method_node = Some(child);
                break;
            }
        }
        let method_node = method_node.expect("Should find method_declaration");

        // Test find_function_declaration
        let identifier = GoLang.find_function_declaration(method_node);
        assert!(identifier.is_some());
        let identifier = identifier.unwrap();
        assert_eq!(identifier.utf8_text(source.as_bytes()).unwrap(), "Method");
    }

    #[test]
    fn test_find_function_declaration_not_function() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser.set_language(&GoLang.tree_sitter_language()).unwrap();

        let source = "package main";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Try with a non-function node
        let result = GoLang.find_function_declaration(root);
        assert!(result.is_none());
    }
}
