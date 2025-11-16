//! Functions related to the interplay between tree-sitter and LSP servers.

use anyhow::Result;
use lsp_types::{
    GotoDefinitionParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};
use std::path::Path;
use tree_sitter::Node;

use crate::lsp::LspServer;

/// Requests go-to-definition from an LSP server for a tree-sitter node
///
/// # Arguments
/// * `lsp_server` - A running LSP server instance
/// * `node` - The tree-sitter node to get the definition for
/// * `file_path` - The path to the file containing the node
///
/// # Returns
/// The LSP GotoDefinition response, which may be None if no definition is found
pub fn goto_definition_for_node(
    lsp_server: &mut LspServer,
    node: &Node,
    file_path: &Path,
) -> Result<Option<lsp_types::GotoDefinitionResponse>> {
    // Get the starting position of the node
    let start = node.start_position();

    // Create the file URI
    let file_uri = format!("file://{}", file_path.display());

    // Create the goto definition parameters
    let params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
            position: Position {
                line: start.row as u32,
                character: start.column as u32,
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    // Send the request and get the response
    lsp_server.request::<lsp_types::request::GotoDefinition>(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Language;
    use crate::parser::{get_calls, parse_file};
    use lsp_types::{
        DidOpenTextDocumentParams, InitializeParams, InitializedParams, TextDocumentItem,
        notification::{DidOpenTextDocument, Initialized},
        request::Initialize,
    };
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_goto_definition_for_node() -> Result<()> {
        // Create a temporary directory for the Swift file
        let temp_dir = TempDir::new()?;
        let file_path = temp_dir.path().join("test.swift");

        // Create a simple Swift program with a function call
        let swift_code = r#"
func foo() {
    print("Hello, Foo")
}

func greet(name: String) {
    print("Hello, \(name)")
}

func main() {
    greet(name: "World")
}
"#;
        fs::write(&file_path, swift_code)?;

        // Parse the file with tree-sitter
        let tree = parse_file(&file_path, Language::Swift)?;

        // Find the greet() call (not the print() call)
        let greet_call = get_calls(&tree)
            .find(|node| {
                node.utf8_text(swift_code.as_bytes())
                    .ok()
                    .map(|text| text.contains("greet"))
                    .unwrap_or(false)
            })
            .expect("Should find the greet call");

        // Start the LSP server
        let mut lsp_server = LspServer::start(
            Language::Swift,
            temp_dir.path().to_path_buf(),
            Default::default(),
        )?;

        // Initialize the LSP server
        let workspace_uri = format!("file://{}", temp_dir.path().display()).parse()?;
        let initialize_params = InitializeParams {
            process_id: Some(std::process::id()),
            workspace_folders: Some(vec![lsp_types::WorkspaceFolder {
                uri: workspace_uri,
                name: "test".to_string(),
            }]),
            ..Default::default()
        };

        lsp_server.request::<Initialize>(initialize_params)?;
        lsp_server.send_notification::<Initialized>(InitializedParams {})?;

        // Open the document
        let file_uri = format!("file://{}", file_path.display());
        lsp_server.send_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
            text_document: TextDocumentItem {
                uri: file_uri.parse()?,
                language_id: "swift".to_string(),
                version: 1,
                text: swift_code.to_string(),
            },
        })?;

        // Request go-to-definition for the call node
        let result = goto_definition_for_node(&mut lsp_server, &greet_call, &file_path)?;

        // Verify the definition points to the correct location
        let response = result.expect("Should find definition for greet function call");

        let lsp_types::GotoDefinitionResponse::Array(locations) = response else {
            panic!("Expected array of locations");
        };
        let location = locations.first().expect("Should have a location");

        // Check that the URI path matches our file
        let location_path_str = location.uri.path().as_str();
        let location_path = std::path::PathBuf::from(location_path_str);
        let canonical_location = location_path.canonicalize().ok();
        let canonical_expected = file_path.canonicalize().ok();
        assert_eq!(
            canonical_location, canonical_expected,
            "Definition should be in the same file"
        );

        // Check that the line number points to the function definition
        // Line 5 is where "func greet(name: String) {" starts
        assert_eq!(
            location.range.start.line, 5,
            "Definition should point to line 1 (the greet function definition)"
        );

        // Stop the LSP server
        lsp_server.stop()?;

        Ok(())
    }
}
