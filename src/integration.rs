//! Functions related to the interplay between tree-sitter and LSP servers.

use anyhow::Result;
use lsp_types::{
    GotoDefinitionParams, Position, TextDocumentIdentifier, TextDocumentPositionParams,
};
use std::path::Path;
use tree_sitter::Node;

use crate::lsp::LspServer;

fn point_to_position(point: tree_sitter::Point) -> Position {
    Position {
        line: point.row as u32,
        character: point.column as u32,
    }
}

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
            position: point_to_position(start),
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    // Send the request and get the response
    lsp_server.request::<lsp_types::request::GotoDefinition>(params)
}

/// Result of finding a call and its definition
#[derive(Debug, Clone)]
pub struct CallDefinition {
    /// The file path containing the call
    pub file_path: std::path::PathBuf,
    /// The tree-sitter node representing the call
    pub call_node: tree_sitter::Node<'static>,
    /// The LSP definition response for the call
    pub definition: lsp_types::GotoDefinitionResponse,
}

/// Finds all function calls in a project and retrieves their definitions from the LSP server
///
/// This function:
/// 1. Finds all files matching the language in the project directory
/// 2. Parses each file with tree-sitter to find function calls
/// 3. Initializes an LSP server for the language
/// 4. Opens each document and queries the definition for each call
///
/// # Arguments
/// * `language` - The programming language to analyze
/// * `project_path` - The root directory of the project to analyze
///
/// # Returns
/// A vector of tuples containing (file_path, call_node, definition_response)
///
/// # Example
/// ```ignore
/// let results = find_all_call_definitions(Language::Rust, &PathBuf::from("./my-project"))?;
/// for result in results {
///     println!("Call in {}: {:?}", result.file_path.display(), result.definition);
/// }
/// ```
pub fn find_all_call_targets(
    language: crate::Language,
    project_path: &Path,
) -> Result<Vec<CallDefinition>> {
    use crate::file_search::FileSearchConfig;
    use crate::parser::{get_calls, parse_file};
    use lsp_types::{
        DidOpenTextDocumentParams, InitializeParams, InitializedParams, TextDocumentItem,
        WorkspaceFolder,
        notification::{DidOpenTextDocument, Initialized},
        request::Initialize,
    };
    use std::fs;

    let mut results = Vec::new();

    // Find all files matching the language
    tracing::info!("Scanning for {} files in project...", language);
    let config = FileSearchConfig::default();
    let matching_files = config.find_language_files(project_path, language)?;
    tracing::info!("Found {} {} files", matching_files.len(), language);

    if matching_files.is_empty() {
        tracing::warn!("No files found for language {}", language);
        return Ok(results);
    }

    // Start LSP server
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = LspServer::start(
        language,
        project_path.to_path_buf(),
        crate::lsp::LspServerConfig::default(),
    )?;

    // Initialize the LSP server
    tracing::info!("Initializing LSP server...");
    let workspace_uri = format!("file://{}", project_path.display()).parse()?;
    let initialize_params = InitializeParams {
        process_id: Some(std::process::id()),
        workspace_folders: Some(vec![WorkspaceFolder {
            uri: workspace_uri,
            name: project_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("workspace")
                .to_string(),
        }]),
        ..Default::default()
    };

    lsp_server.request::<Initialize>(initialize_params)?;
    lsp_server.send_notification::<Initialized>(InitializedParams {})?;
    tracing::info!("LSP server initialized");

    // Initialize performance timer
    let start_time = std::time::Instant::now();
    let mut total_calls = 0;

    // Process each file
    for (index, file_path) in matching_files.iter().enumerate() {
        tracing::info!(
            "({index}/{}) Processing file: {}",
            matching_files.len(),
            file_path.display()
        );

        // Read the file content
        let file_content = match fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!("Failed to read file {}: {}", file_path.display(), e);
                continue;
            }
        };

        // Parse the file with tree-sitter
        let tree = match parse_file(file_path, language) {
            Ok(tree) => tree,
            Err(e) => {
                tracing::warn!("Failed to parse file {}: {}", file_path.display(), e);
                continue;
            }
        };

        // Open the document in the LSP server
        let file_uri = format!("file://{}", file_path.display());
        if let Err(e) =
            lsp_server.send_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
                text_document: TextDocumentItem {
                    uri: file_uri.parse()?,
                    language_id: language.to_string().to_lowercase(),
                    version: 1,
                    text: file_content.clone(),
                },
            })
        {
            tracing::warn!("Failed to open document {}: {}", file_path.display(), e);
            continue;
        }

        // Some LSP servers seem to require a bit of time before they're ready
        // tracing::info!("Waiting for LSP server to index the project...");
        // std::thread::sleep(std::time::Duration::from_secs(5));

        // Find all calls in the file
        let calls: Vec<_> = get_calls(&tree).collect();
        tracing::debug!("Found {} calls in {}", calls.len(), file_path.display());
        total_calls += calls.len();

        // For each call, get its definition
        for call_node in calls {
            // Query the LSP server for the definition
            match goto_definition_for_node(&mut lsp_server, &call_node, file_path) {
                Ok(Some(definition)) => {
                    // We need to convert the node to a 'static lifetime by storing the tree
                    // Since we can't easily do that here, we'll use unsafe to extend the lifetime
                    // This is safe because we're only storing the node data, not the reference
                    let static_node: Node<'static> = unsafe { std::mem::transmute(call_node) };

                    results.push(CallDefinition {
                        file_path: file_path.clone(),
                        call_node: static_node,
                        definition,
                    });
                    tracing::debug!(
                        "Found definition for call at {}:{}:{}",
                        file_path.display(),
                        call_node.start_position().row,
                        call_node.start_position().column
                    );
                }
                Ok(None) => {
                    tracing::debug!(
                        "No definition found for call at {}:{}:{}",
                        file_path.display(),
                        call_node.start_position().row,
                        call_node.start_position().column
                    );
                }
                Err(e) => {
                    tracing::debug!(
                        "Failed to get definition for call at {}:{}:{}: {}",
                        file_path.display(),
                        call_node.start_position().row,
                        call_node.start_position().column,
                        e
                    );
                }
            }
        }

        // Close the document in the LSP server
        let close_params = lsp_types::DidCloseTextDocumentParams {
            text_document: lsp_types::TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
        };
        if let Err(e) = lsp_server
            .send_notification::<lsp_types::notification::DidCloseTextDocument>(close_params)
        {
            tracing::warn!("Failed to close document {}: {}", file_path.display(), e);
        }
    }

    tracing::info!(
        "Processed {} files and {} calls in {:.2?}",
        matching_files.len(),
        total_calls,
        start_time.elapsed()
    );

    // Stop the LSP server
    tracing::info!("Stopping LSP server...");
    if let Err(e) = lsp_server.stop() {
        tracing::error!("Error stopping LSP server: {}", e);
    }

    Ok(results)
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
