//! This module provides utilities for parsing source code using tree-sitter.
//!
//! It includes support for finding calls across multiple programming languages.

use anyhow::Result;
use std::fs;
use std::path::Path;
use tree_sitter::{Node, Parser, Tree, TreeCursor};

use crate::Language;

/// Parses source code content using Tree Sitter for the specified language
///
/// # Arguments
/// * `source_code` - The source code string to parse
/// * `language` - The programming language of the source code
///
/// # Returns
/// * `Result<Tree>` - The parsed syntax tree or an error
pub fn parse_file_content(source_code: &str, language: Language) -> Result<Tree> {
    // Create a parser
    let mut parser = Parser::new();

    // Set the language-specific grammar
    let ts_language = get_tree_sitter_language(language)?;
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
pub fn parse_file(file_path: &Path, language: Language) -> Result<Tree> {
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

/// Returns the Tree Sitter language grammar for the given language
fn get_tree_sitter_language(language: Language) -> Result<tree_sitter::Language> {
    match language {
        Language::Rust => Ok(tree_sitter_rust::LANGUAGE.into()),
        Language::Python => Ok(tree_sitter_python::LANGUAGE.into()),
        Language::TypeScript => Ok(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        Language::Go => Ok(tree_sitter_go::LANGUAGE.into()),
        Language::Swift => Ok(tree_sitter_swift::LANGUAGE.into()),
    }
}

/// Returns an iterator over all function and method calls in the syntax tree
///
/// This function traverses the entire tree and yields nodes that represent
/// function calls, method calls, or similar call expressions. The specific
/// node kinds matched depend on the language being parsed.
///
/// # Arguments
/// * `tree` - The parsed syntax tree to search
///
/// # Returns
/// An iterator that yields `Node` for each call found in the tree
///
/// # Example
/// ```ignore
/// let tree = parse_file(path, Language::Rust)?;
/// for call in get_calls(&tree) {
///     println!("Found call: {:?}", call.kind());
/// }
/// ```
pub fn get_calls(tree: &Tree) -> impl Iterator<Item = Node<'_>> {
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

/// Iterator that traverses a Tree-sitter tree and yields call nodes
struct CallIterator<'a> {
    cursor: TreeCursor<'a>,
    call_kinds: &'a [&'a str],
    visited_root: bool,
}

impl<'a> Iterator for CallIterator<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let node = self.cursor.node();

            // Check if current node is a call
            if self.visited_root && self.call_kinds.contains(&node.kind()) {
                // Move to next node for the next iteration
                if !self.cursor.goto_first_child() {
                    while !self.cursor.goto_next_sibling() {
                        if !self.cursor.goto_parent() {
                            return Some(node);
                        }
                    }
                }

                return Some(node);
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

        let tree = parse_file(temp_file.path(), Language::Rust)?;
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

        let tree = parse_file(temp_file.path(), Language::Python)?;
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

        let tree = parse_file(temp_file.path(), Language::TypeScript)?;
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

        let tree = parse_file(temp_file.path(), Language::Go)?;
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

        let tree = parse_file(temp_file.path(), Language::Swift)?;
        let root = tree.root_node();

        // Check that we got a valid tree
        assert!(root.child_count() > 0);
        assert_eq!(root.kind(), "source_file");

        Ok(())
    }

    #[test]
    fn test_parse_nonexistent_file() {
        let result = parse_file(Path::new("/nonexistent/file.rs"), Language::Rust);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_syntax() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "fn main() {{ this is invalid rust")?;

        // Parser should still succeed but might have error nodes
        let tree = parse_file(temp_file.path(), Language::Rust)?;
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
        let tree = parse_file(temp_file.path(), Language::Rust)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: println! (macro), calculate (call), foo (call)
        assert_eq!(calls.len(), 3);

        // Verify order and content
        assert_eq!(calls[0].kind(), "macro_invocation");
        assert!(calls[0].utf8_text(&source)?.contains("println!"));

        assert_eq!(calls[1].kind(), "call_expression");
        assert!(calls[1].utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].kind(), "call_expression");
        assert!(calls[2].utf8_text(&source)?.contains("foo"));

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
        let tree = parse_file(temp_file.path(), Language::Python)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: print, calculate, foo in that order
        assert_eq!(calls.len(), 3);

        assert_eq!(calls[0].kind(), "call");
        assert!(calls[0].utf8_text(&source)?.contains("print"));

        assert_eq!(calls[1].kind(), "call");
        assert!(calls[1].utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].kind(), "call");
        assert!(calls[2].utf8_text(&source)?.contains("foo"));

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
        let tree = parse_file(temp_file.path(), Language::TypeScript)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: console.log, calculate, new MyClass in that order
        assert_eq!(calls.len(), 3);

        assert_eq!(calls[0].kind(), "call_expression");
        assert!(calls[0].utf8_text(&source)?.contains("console.log"));

        assert_eq!(calls[1].kind(), "call_expression");
        assert!(calls[1].utf8_text(&source)?.contains("calculate"));

        assert_eq!(calls[2].kind(), "new_expression");
        assert!(calls[2].utf8_text(&source)?.contains("MyClass"));

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
        let tree = parse_file(temp_file.path(), Language::Go)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: println, calculate in that order
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].kind(), "call_expression");
        assert!(calls[0].utf8_text(&source)?.contains("println"));

        assert_eq!(calls[1].kind(), "call_expression");
        assert!(calls[1].utf8_text(&source)?.contains("calculate"));

        Ok(())
    }

    #[test]
    fn test_get_calls_swift() -> Result<()> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "func main() {{")?;
        writeln!(temp_file, "    print(\"Hello\")")?;
        writeln!(temp_file, "    let x = calculate(5, 10)")?;
        writeln!(temp_file, "}}")?;

        let source = fs::read(temp_file.path())?;
        let tree = parse_file(temp_file.path(), Language::Swift)?;
        let calls: Vec<_> = get_calls(&tree).collect();

        // Should find: print, calculate in that order
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].kind(), "call_expression");
        assert!(calls[0].utf8_text(&source)?.contains("print"));

        assert_eq!(calls[1].kind(), "call_expression");
        assert!(calls[1].utf8_text(&source)?.contains("calculate"));

        Ok(())
    }
}
