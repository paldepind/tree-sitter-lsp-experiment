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
}

impl std::fmt::Display for SwiftLang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}
