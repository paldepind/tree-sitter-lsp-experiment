//! This module provides utilities for parsing source code using tree-sitter.
//!
//! It includes support for finding calls across multiple programming languages.

use anyhow::Result;
use std::fmt::Display;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

use crate::{call_node::CallNode, language::Language};

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

struct DisplayNodeLocation<'a> {
    file_path: &'a Path,
    node: Node<'a>,
}

impl Display for DisplayNodeLocation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let row = self.node.start_position().row + 1;
        let column = self.node.start_position().column + 1;
        write!(f, "{}:{}:{}", self.file_path.display(), row, column)
    }
}

pub fn display_node_location<'a>(file_path: &'a Path, node: Node<'a>) -> impl 'a + Display {
    DisplayNodeLocation { file_path, node }
}

/// Returns an iterator over all function and method calls in the syntax tree
///
/// This function traverses the entire tree and yields CallNode instances that represent
/// function calls, method calls, or similar call expressions. The specific
/// node kinds matched depend on the language being parsed.
///
/// # Arguments
/// * `tree` - The parsed syntax tree to search
/// * `language` - The programming language of the tree
///
/// # Returns
/// An iterator that yields `CallNode` for each call found in the tree
///
/// # Example
/// ```ignore
/// let tree = parse_file(path, RustLang)?;
/// for call in get_calls(&tree, RustLang) {
///     println!("Found call: {:?}", call.call_node.kind());
/// }
/// ```
pub fn get_calls(tree: &Tree, language: impl Language) -> impl Iterator<Item = CallNode<'_>> {
    CallIterator {
        cursor: tree.walk(),
        language,
        visited_root: false,
    }
}

/// Iterator that traverses a Tree-sitter tree and yields call nodes
struct CallIterator<'a, L: Language> {
    cursor: TreeCursor<'a>,
    language: L,
    visited_root: bool,
}

impl<'a, L: Language> Iterator for CallIterator<'a, L> {
    type Item = CallNode<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.cursor.node();

            // Check if current node is a call using the language-specific method
            if self.visited_root {
                if let Some(goto_definition_node) = self.language.find_call(node) {
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
        let calls: Vec<_> = get_calls(&tree, crate::RustLang).collect();

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
        let calls: Vec<_> = get_calls(&tree, crate::PythonLang).collect();

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
        let calls: Vec<_> = get_calls(&tree, crate::TypeScriptLang).collect();

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
        let calls: Vec<_> = get_calls(&tree, crate::GoLang).collect();

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
        let calls: Vec<_> = get_calls(&tree, crate::SwiftLang).collect();

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
        writeln!(
            temp_file,
            "let digest = Foo<UInt8>.allocate(capacity: length)"
        )?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), crate::SwiftLang)?;
        let calls: Vec<_> = get_calls(&tree, crate::SwiftLang).collect();

        assert!(calls.len() == 3);

        // Find the method call 'calc.add(2, 3)'
        let method_call = calls.get(1).expect("Method call not found");
        assert_eq!(method_call.call_node.kind(), "call_expression");
        // The goto_definition_node should point to just the method name "add"
        assert_eq!(method_call.goto_definition_node.kind(), "simple_identifier");
        let def_text = method_call.goto_definition_node.utf8_text(&source)?;
        assert_eq!(def_text, "add");

        // Find the method call `Foo<UInt8>.allocate(capacity: length)`
        let method_call = calls.get(2).expect("Method call not found");
        assert_eq!(method_call.call_node.kind(), "call_expression");
        // The goto_definition_node should point to just the method name
        // "allocate", but the Swift tree-sitter grammar doesn't parse the call
        // correctly due to the generics. We might want to work around this in
        // the future.
        let def_text = method_call.goto_definition_node.utf8_text(&source)?;
        assert_eq!(def_text, ".allocate(capacity: length)");

        Ok(())
    }
}
