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

    fn find_function_declaration<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Check if this is a function item
        if node.kind() != "function_item" {
            return None;
        }

        // Find the identifier child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "identifier")
    }

    fn call_hierarchy_target<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Valid targets for call hierarchy in Rust:
        // - function_item (top-level functions and associated functions)
        // - function_signature_item (trait methods)
        match node.kind() {
            "function_item" => self.find_function_declaration(node),
            "function_signature_item" => {
                // For trait method signatures, find the identifier
                let mut cursor = node.walk();
                node.children(&mut cursor)
                    .find(|&child| child.kind() == "identifier")
            }
            _ => None,
        }
    }
}

impl std::fmt::Display for RustLang {
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
        parser
            .set_language(&RustLang.tree_sitter_language())
            .unwrap();

        let source = "fn hello() { println!(\"Hello\"); }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Find the function_item node
        let mut cursor = root.walk();
        let function_node = root
            .children(&mut cursor)
            .find(|n| n.kind() == "function_item")
            .expect("Should find function_item");

        // Test find_function_declaration
        let identifier = RustLang.find_function_declaration(function_node);
        assert!(identifier.is_some());
        let identifier = identifier.unwrap();
        assert_eq!(identifier.kind(), "identifier");
        assert_eq!(identifier.utf8_text(source.as_bytes()).unwrap(), "hello");
    }

    #[test]
    fn test_find_function_declaration_not_function() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&RustLang.tree_sitter_language())
            .unwrap();

        let source = "let x = 5;";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Try with a non-function node
        let result = RustLang.find_function_declaration(root);
        assert!(result.is_none());
    }

    #[test]
    fn test_call_hierarchy_target_function() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&RustLang.tree_sitter_language())
            .unwrap();

        let source = "fn hello() { println!(\"Hello\"); }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Find the function_item node
        let mut cursor = root.walk();
        let function_node = root
            .children(&mut cursor)
            .find(|n| n.kind() == "function_item")
            .expect("Should find function_item");

        // Test call_hierarchy_target
        let target = RustLang.call_hierarchy_target(function_node);
        assert!(target.is_some());
        assert_eq!(target.unwrap().kind(), "identifier");
    }

    #[test]
    fn test_call_hierarchy_target_trait_method() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&RustLang.tree_sitter_language())
            .unwrap();

        let source = "trait MyTrait { fn method(&self); }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Find the function_signature_item node
        let mut found_signature = None;
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "trait_item" {
                let mut trait_cursor = child.walk();
                for trait_child in child.children(&mut trait_cursor) {
                    if trait_child.kind() == "declaration_list" {
                        let mut decl_cursor = trait_child.walk();
                        for decl_child in trait_child.children(&mut decl_cursor) {
                            if decl_child.kind() == "function_signature_item" {
                                found_signature = Some(decl_child);
                                break;
                            }
                        }
                    }
                }
            }
        }

        let signature_node = found_signature.expect("Should find function_signature_item");

        // Test call_hierarchy_target
        let target = RustLang.call_hierarchy_target(signature_node);
        assert!(target.is_some());
        assert_eq!(target.unwrap().kind(), "identifier");
    }

    #[test]
    fn test_call_hierarchy_target_not_function() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&RustLang.tree_sitter_language())
            .unwrap();

        let source = "let x = 5;";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Try with a non-function node
        let result = RustLang.call_hierarchy_target(root);
        assert!(result.is_none());
    }
}
