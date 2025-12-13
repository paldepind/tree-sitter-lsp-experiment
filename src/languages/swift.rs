//! Swift language implementation.

use crate::language::Language;
use tree_sitter::Node;

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

    fn call_node_kinds(&self) -> &'static [&'static str] {
        &["call_expression", "function_call_expression"]
    }

    fn find_call<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        if !self.call_node_kinds().contains(&node.kind()) {
            return None;
        }

        // For Swift method calls, find the method name
        if node.kind() == "call_expression" {
            // Look for a child that represents the method/function being called
            let mut cursor = node.walk();

            // Check if this is a method call (has a navigation expression like obj.method)
            for child in node.children(&mut cursor) {
                // In Swift, method calls have a structure like:
                // call_expression
                //   navigation_expression (e.g., "calc.add")
                //     simple_identifier ("calc")
                //     navigation_suffix
                //       simple_identifier ("add") <- This is what we want
                if child.kind() == "navigation_expression" {
                    // Find the navigation suffix which contains the method name
                    let mut nav_cursor = child.walk();
                    for nav_child in child.children(&mut nav_cursor) {
                        if nav_child.kind() == "navigation_suffix" {
                            // Find the identifier within the suffix
                            let mut suffix_cursor = nav_child.walk();
                            for suffix_child in nav_child.children(&mut suffix_cursor) {
                                if suffix_child.kind() == "simple_identifier" {
                                    return Some(suffix_child);
                                }
                            }
                        }
                    }
                }
                // For simple function calls (not method calls), look for the function name directly
                else if child.kind() == "simple_identifier" || child.kind() == "identifier" {
                    return Some(child);
                }
            }
        }

        // Default: return the call node itself
        Some(node)
    }

    fn find_function_declaration<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Check if this is a function declaration
        if node.kind() != "function_declaration" {
            return None;
        }

        // Find the simple_identifier child
        let mut cursor = node.walk();
        node.children(&mut cursor)
            .find(|&child| child.kind() == "simple_identifier")
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
    fn test_find_function_declaration() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&SwiftLang.tree_sitter_language())
            .unwrap();

        let source = "func hello() { print(\"Hello\") }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Find the function_declaration node
        let mut cursor = root.walk();
        let function_node = root
            .children(&mut cursor)
            .find(|n| n.kind() == "function_declaration")
            .expect("Should find function_declaration");

        // Test find_function_declaration
        let identifier = SwiftLang.find_function_declaration(function_node);
        assert!(identifier.is_some());
        let identifier = identifier.unwrap();
        assert_eq!(identifier.kind(), "simple_identifier");
        assert_eq!(identifier.utf8_text(source.as_bytes()).unwrap(), "hello");
    }

    #[test]
    fn test_find_method_declaration() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&SwiftLang.tree_sitter_language())
            .unwrap();

        let source = "class MyClass { func myMethod() {} }";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Manually find the class_declaration, then the function_declaration inside
        let mut cursor = root.walk();
        let mut class_node = None;
        for child in root.children(&mut cursor) {
            if child.kind() == "class_declaration" {
                class_node = Some(child);
                break;
            }
        }
        let class_node = class_node.expect("Should find class_declaration");

        // Find function_declaration inside class body
        let mut cursor = class_node.walk();
        let mut method_node = None;
        for child in class_node.children(&mut cursor) {
            if child.kind() == "class_body" {
                let mut body_cursor = child.walk();
                for body_child in child.children(&mut body_cursor) {
                    if body_child.kind() == "function_declaration" {
                        method_node = Some(body_child);
                        break;
                    }
                }
                break;
            }
        }
        let method_node = method_node.expect("Should find function_declaration");

        // Test find_function_declaration
        let identifier = SwiftLang.find_function_declaration(method_node);
        assert!(identifier.is_some());
        let identifier = identifier.unwrap();
        assert_eq!(identifier.kind(), "simple_identifier");
        assert_eq!(identifier.utf8_text(source.as_bytes()).unwrap(), "myMethod");
    }

    #[test]
    fn test_find_function_declaration_not_function() {
        use tree_sitter::Parser;

        let mut parser = Parser::new();
        parser
            .set_language(&SwiftLang.tree_sitter_language())
            .unwrap();

        let source = "let x = 5";
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Try with a non-function node
        let result = SwiftLang.find_function_declaration(root);
        assert!(result.is_none());
    }
}
