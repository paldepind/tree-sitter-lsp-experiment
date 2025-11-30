//! This module provides utilities for parsing source code using tree-sitter.
//!
//! It includes support for finding calls across multiple programming languages.

use anyhow::Result;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

use crate::language::Language;

/// Parses source code content using Tree Sitter for the specified language
///
/// # Arguments
/// * `source_code` - The source code string to parse
/// * `language` - The programming language of the source code
///
/// # Returns
/// * `Result<Tree>` - The parsed syntax tree or an error
pub fn parse_file_content(source_code: &str, language: impl Language) -> Result<Tree> {
    // Create a parser
    let mut parser = Parser::new();

    // Set the language-specific grammar
    let ts_language = language.tree_sitter_language();
    parser
        .set_language(&ts_language)
        .map_err(|e| anyhow::anyhow!("Failed to set language for parser: {}", e))?;

    // Parse the source code
    let tree = parser
        .parse(source_code, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse source code"))?;

    tracing::debug!(
        "Successfully parsed source code ({} nodes in tree)",
        tree.root_node().descendant_count()
    );

    Ok(tree)
}

/// Parses a file using Tree Sitter for the specified language
///
/// # Arguments
/// * `file_path` - Path to the file to parse
/// * `language` - The programming language of the file
///
/// # Returns
/// * `Result<Tree>` - The parsed syntax tree or an error
pub fn parse_file(file_path: &Path, language: impl Language) -> Result<Tree> {
    // Read the file contents
    let source_code = fs::read_to_string(file_path)
        .map_err(|e| anyhow::anyhow!("Failed to read file {}: {}", file_path.display(), e))?;

    // Parse using the content parser
    let tree = parse_file_content(&source_code, language)
        .map_err(|e| anyhow::anyhow!("Failed to parse file {}: {}", file_path.display(), e))?;

    tracing::debug!(
        "Successfully parsed {} ({} nodes in tree)",
        file_path.display(),
        tree.root_node().descendant_count()
    );

    Ok(tree)
}

pub struct CallNode<'tree> {
    // The node representing the function/method call
    pub call_node: Node<'tree>,
    // The node for which goto definiton should be performed
    pub goto_definition_node: Node<'tree>,
}

impl<'tree> CallNode<'tree> {
    /// Pretty prints the call node with visual indicators for the call and goto definition ranges
    ///
    /// This method displays the source line with underline markers showing where the call
    /// and goto definition nodes are located. If the nodes span multiple lines, it returns
    /// a simple multi-line indicator instead.
    ///
    /// # Arguments
    /// * `source_lines` - All lines of source code as a slice of string slices
    ///
    /// # Returns
    /// A vector of strings representing the pretty-printed output, or None if the call
    /// spans multiple lines
    pub fn pretty_print(&self, source_lines: &[&str]) -> Option<Vec<String>> {
        let line_num = self.call_node.start_position().row;
        let call_start_col = self.call_node.start_position().column;
        let call_end_col = self.call_node.end_position().column;
        let goto_start_col = self.goto_definition_node.start_position().column;
        let goto_end_col = self.goto_definition_node.end_position().column;

        // Only show if both call and goto are on the same line
        if self.call_node.start_position().row == self.call_node.end_position().row
            && self.goto_definition_node.start_position().row
                == self.goto_definition_node.end_position().row
            && self.call_node.start_position().row == self.goto_definition_node.start_position().row
            && let Some(source_line) = source_lines.get(line_num)
        {
            let mut output = Vec::new();

            // Source line with line number
            output.push(format!("{}: {}", line_num + 1, source_line));

            // Create underline for call node
            let mut call_underline = String::new();
            call_underline.push_str(" ".repeat(call_start_col).as_str());
            call_underline.push_str("^".repeat(call_end_col - call_start_col).as_str());

            // Create underline for goto definition node
            let mut goto_underline = String::new();
            goto_underline.push_str(" ".repeat(goto_start_col).as_str());
            goto_underline.push_str("~".repeat(goto_end_col - goto_start_col).as_str());

            // Print with proper indentation (matching line number width)
            let indent = " ".repeat(format!("{}", line_num + 1).len() + 2);
            output.push(format!("{}{} call", indent, call_underline));
            output.push(format!("{}{} goto definition", indent, goto_underline));

            Some(output)
        } else {
            None
        }
    }
}

/// Returns an iterator over all function and method calls in the syntax tree
///
/// This function traverses the entire tree and yields CallNode instances that represent
/// function calls, method calls, or similar call expressions. The specific
/// node kinds matched depend on the language being parsed.
///
/// # Arguments
/// * `tree` - The parsed syntax tree to search
///
/// # Returns
/// An iterator that yields `CallNode` for each call found in the tree
///
/// # Example
/// ```ignore
/// let tree = parse_file(path, &RustLang)?;
/// for call in get_calls(&tree) {
///     println!("Found call: {:?}", call.call_node.kind());
/// }
/// ```
pub fn get_calls(tree: &Tree) -> impl Iterator<Item = CallNode<'_>> {
    // Node kinds that represent calls in different languages
    const CALL_NODE_KINDS: &[&str] = &[
        // Rust
        "call_expression",
        "macro_invocation",
        // Python
        "call",
        // TypeScript/JavaScript
        "call_expression",
        "new_expression",
        // Go
        "call_expression",
        // Swift
        "call_expression",
        "function_call_expression",
    ];

    CallIterator {
        cursor: tree.walk(),
        call_kinds: CALL_NODE_KINDS,
        visited_root: false,
    }
}

/// Finds the appropriate node for goto definition within a call node
/// For method calls, this returns the method name node; otherwise returns the call node itself
fn find_goto_definition_node<'a>(call_node: Node<'a>) -> Node<'a> {
    // For Swift method calls, find the method name
    if call_node.kind() == "call_expression" {
        // Look for a child that represents the method/function being called
        let mut cursor = call_node.walk();

        // Check if this is a method call (has a navigation expression like obj.method)
        for child in call_node.children(&mut cursor) {
            // In Swift, method calls have a structure like:
            // call_expression
            //   navigation_expression (e.g., "calc.add")
            //     simple_identifier ("calc")
            //     navigation_suffix
            //       simple_identifier ("add") <- This is what we want
            // (call_expression ; [5, 17] - [5, 31]
            //   (navigation_expression ; [5, 17] - [5, 25]
            //     target: (simple_identifier) ; [5, 17] - [5, 21]
            //     suffix: (navigation_suffix ; [5, 21] - [5, 25]
            //       suffix: **(simple_identifier)**)) ; [5, 22] - [5, 25]
            //   (call_suffix ; [5, 25] - [5, 31]
            //     (value_arguments ; [5, 25] - [5, 31]
            //       (value_argument ; [5, 26] - [5, 27]
            //         value: (integer_literal)) ; [5, 26] - [5, 27]
            //       (value_argument ; [5, 29] - [5, 30]
            //         value: (integer_literal)))))
            if child.kind() == "navigation_expression" {
                // Find the navigation suffix which contains the method name
                let mut nav_cursor = child.walk();
                for nav_child in child.children(&mut nav_cursor) {
                    if nav_child.kind() == "navigation_suffix" {
                        // Find the identifier within the suffix
                        let mut suffix_cursor = nav_child.walk();
                        for suffix_child in nav_child.children(&mut suffix_cursor) {
                            if suffix_child.kind() == "simple_identifier" {
                                return suffix_child;
                            }
                        }
                    }
                }
            }
            // For simple function calls (not method calls), look for the function name directly
            else if child.kind() == "simple_identifier" || child.kind() == "identifier" {
                return child;
            }
        }
    }

    // Default: return the call node itself
    call_node
}

/// Iterator that traverses a Tree-sitter tree and yields call nodes
struct CallIterator<'a> {
    cursor: TreeCursor<'a>,
    call_kinds: &'a [&'a str],
    visited_root: bool,
}

impl<'a> Iterator for CallIterator<'a> {
    type Item = CallNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.cursor.node();

            // Check if current node is a call
            if self.visited_root && self.call_kinds.contains(&node.kind()) {
                // Find the appropriate node for goto definition
                let goto_definition_node = find_goto_definition_node(node);
                let call_node = CallNode {
                    call_node: node,
                    goto_definition_node,
                };

                // Move to next node for the next iteration
                if !self.cursor.goto_first_child() {
                    while !self.cursor.goto_next_sibling() {
                        if !self.cursor.goto_parent() {
                            return Some(call_node);
                        }
                    }
                }

                return Some(call_node);
            }

            self.visited_root = true;

            // Traverse the tree depth-first
            if self.cursor.goto_first_child() {
                continue;
            }

            loop {
                if self.cursor.goto_next_sibling() {
                    break;
                }

                if !self.cursor.goto_parent() {
                    return None; // Reached end of tree
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_rust_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "fn main() {{")?;
        writeln!(temp_file, "    println!(\"Hello, world!\");")?;
        writeln!(temp_file, "}}")?;

        let tree = parse_file(temp_file.path(), crate::RustLang)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "source_file");

        Ok(())
    }

    #[test]
    fn test_parse_python_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "def hello():")?;
        writeln!(temp_file, "    print('Hello, world!')")?;

        let tree = parse_file(temp_file.path(), crate::PythonLang)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "module");

        Ok(())
    }

    #[test]
    fn test_parse_typescript_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "function hello(): void {{")?;
        writeln!(temp_file, "    console.log('Hello, world!');")?;
        writeln!(temp_file, "}}")?;

        let tree = parse_file(temp_file.path(), crate::TypeScriptLang)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "program");

        Ok(())
    }

    #[test]
    fn test_parse_go_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "package main")?;
        writeln!(temp_file)?;
        writeln!(temp_file, "func main() {{")?;
        writeln!(temp_file, "    println(\"Hello, world!\")")?;
        writeln!(temp_file, "}}")?;

        let tree = parse_file(temp_file.path(), crate::GoLang)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "source_file");

        Ok(())
    }

    #[test]
    fn test_parse_swift_file() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "func hello() {{")?;
        writeln!(temp_file, "    print(\"Hello, world!\")")?;
        writeln!(temp_file, "}}")?;

        let tree = parse_file(temp_file.path(), crate::SwiftLang)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "source_file");

        Ok(())
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let result = parse_file(Path::new("/nonexistent/file.rs"), crate::RustLang);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_syntax() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "fn main() {{ this is invalid rust")?;

        // Parser should still succeed but might have error nodes
        let tree = parse_file(temp_file.path(), crate::RustLang)?;
        let root = tree.root_node();

        // Tree should still be created even with errors
        assert!(root.child_count() > 0);

        Ok(())
    }

    #[test]
    fn test_get_calls_rust() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "fn main() {{")?;
        writeln!(temp_file, "    println!(\"Hello\");")?;
        writeln!(temp_file, "    let x = calculate(5, 10);")?;
        writeln!(temp_file, "    foo();")?;
        writeln!(temp_file, "}}")?;
        writeln!(temp_file)?;
        writeln!(temp_file, "fn calculate(a: i32, b: i32) -> i32 {{")?;
        writeln!(temp_file, "    a + b")?;
        writeln!(temp_file, "}}")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::RustLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: println! (macro), calculate (call), foo (call)
        assert_eq!(calls.len(), 3);

        // Verify order and content
        assert_eq!(calls[0].call_node.kind(), "macro_invocation");
        assert!(calls[0].call_node.utf8_text(&source)?.contains("println!"));

        assert_eq!(calls[1].call_node.kind(), "call_expression");
        assert!(calls[1].call_node.utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].call_node.kind(), "call_expression");
        assert!(calls[2].call_node.utf8_text(&source)?.contains("foo"));

        Ok(())
    }

    #[test]
    fn test_get_calls_python() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "def main():")?;
        writeln!(temp_file, "    print('Hello')")?;
        writeln!(temp_file, "    result = calculate(5, 10)")?;
        writeln!(temp_file, "    foo()")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::PythonLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: print, calculate, foo in that order
        assert_eq!(calls.len(), 3);

        assert_eq!(calls[0].call_node.kind(), "call");
        assert!(calls[0].call_node.utf8_text(&source)?.contains("print"));

        assert_eq!(calls[1].call_node.kind(), "call");
        assert!(calls[1].call_node.utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].call_node.kind(), "call");
        assert!(calls[2].call_node.utf8_text(&source)?.contains("foo"));

        Ok(())
    }

    #[test]
    fn test_get_calls_typescript() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "function main() {{")?;
        writeln!(temp_file, "    console.log('Hello');")?;
        writeln!(temp_file, "    const x = calculate(5, 10);")?;
        writeln!(temp_file, "    const obj = new MyClass();")?;
        writeln!(temp_file, "}}")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::TypeScriptLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: console.log, calculate, new MyClass in that order
        assert_eq!(calls.len(), 3);

        assert_eq!(calls[0].call_node.kind(), "call_expression");
        assert!(
            calls[0]
                .call_node
                .utf8_text(&source)?
                .contains("console.log")
        );

        assert_eq!(calls[1].call_node.kind(), "call_expression");
        assert!(calls[1].call_node.utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].call_node.kind(), "new_expression");
        assert!(calls[2].call_node.utf8_text(&source)?.contains("MyClass"));

        Ok(())
    }

    #[test]
    fn test_get_calls_go() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "package main")?;
        writeln!(temp_file)?;
        writeln!(temp_file, "func main() {{")?;
        writeln!(temp_file, "    println(\"Hello\")")?;
        writeln!(temp_file, "    x := calculate(5, 10)")?;
        writeln!(temp_file, "}}")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::GoLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: println, calculate in that order
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].call_node.kind(), "call_expression");
        assert!(calls[0].call_node.utf8_text(&source)?.contains("println"));

        assert_eq!(calls[1].call_node.kind(), "call_expression");
        assert!(calls[1].call_node.utf8_text(&source)?.contains("calculate"));

        Ok(())
    }

    #[test]
    fn test_get_calls_swift_function_call() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "func main() {{")?;
        writeln!(temp_file, "    print(\"Hello\")")?;
        writeln!(temp_file, "    let x = calculate(5, 10)")?;
        writeln!(temp_file, "}}")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::SwiftLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: print, calculate in that order
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].call_node.kind(), "call_expression");
        assert!(calls[0].call_node.utf8_text(&source)?.contains("print"));
        assert_eq!(calls[0].goto_definition_node.kind(), "simple_identifier");
        let def_text = calls[0].goto_definition_node.utf8_text(&source)?;
        assert_eq!(def_text, "print");

        assert_eq!(calls[1].call_node.kind(), "call_expression");
        assert_eq!(calls[1].call_node.utf8_text(&source)?, "calculate(5, 10)");
        assert_eq!(calls[1].goto_definition_node.kind(), "simple_identifier");
        let def_text = calls[1].goto_definition_node.utf8_text(&source)?;
        assert_eq!(def_text, "calculate");

        Ok(())
    }

    #[test]
    fn test_get_calls_swift_method_call() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "class Calculator {{")?;
        writeln!(temp_file, "    func add(_ a: Int, _ b: Int) -> Int {{")?;
        writeln!(temp_file, "        return a + b")?;
        writeln!(temp_file, "    }}")?;
        writeln!(temp_file, "}}")?;
        writeln!(temp_file, "let calc = Calculator()")?;
        writeln!(temp_file, "let result = calc.add(2, 3)")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::SwiftLang)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find the Calculator() constructor call and calc.add(2, 3) method call
        assert!(calls.len() == 2);

        // Find the method call 'calc.add(2, 3)'
        let method_call = calls
            .iter()
            .find(|c| c.call_node.utf8_text(&source).unwrap().contains("add"))
            .expect("Method call not found");

        // Verify it's a call_expression
        assert_eq!(method_call.call_node.kind(), "call_expression");

        // The goto_definition_node should point to just the method name "add"
        assert_eq!(method_call.goto_definition_node.kind(), "simple_identifier");
        let def_text = method_call.goto_definition_node.utf8_text(&source)?;
        assert_eq!(def_text, "add");

        Ok(())
    }
}
