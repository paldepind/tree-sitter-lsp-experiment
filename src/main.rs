use anyhow::Result;
use lsp_types::{
    GotoDefinitionParams, GotoDefinitionResponse, InitializeParams, Position,
    TextDocumentIdentifier, TextDocumentPositionParams, WorkspaceFolder,
    request::{GotoDefinition, Initialize, Request},
};
use serde_json::{from_value, to_value};
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::str::FromStr;
use tree_sitter_lsp_experiment::{FileFinder, Language, start_lsp_server};

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <project_path> --language <language>", args[0]);
        eprintln!("Supported languages: rust, python, typescript, go");
        std::process::exit(1);
    }

    let mut project_path: Option<PathBuf> = None;
    let mut language: Option<Language> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--language" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --language requires a value");
                    std::process::exit(1);
                }
                language = Some(Language::from_str(&args[i + 1])?);
                i += 2;
            }
            arg if !arg.starts_with('-') => {
                if project_path.is_none() {
                    project_path = Some(PathBuf::from(arg));
                    i += 1;
                } else {
                    eprintln!("Error: Multiple project paths provided");
                    std::process::exit(1);
                }
            }
            _ => {
                eprintln!("Error: Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    let project_path = match project_path {
        Some(path) => path,
        None => {
            eprintln!("Error: Project path is required");
            eprintln!("Usage: {} <project_path> --language <language>", args[0]);
            std::process::exit(1);
        }
    };

    let language = match language {
        Some(lang) => lang,
        None => {
            eprintln!("Error: --language argument is required");
            eprintln!("Usage: {} <project_path> --language <language>", args[0]);
            eprintln!("Supported languages: rust, python, typescript, go");
            std::process::exit(1);
        }
    };

    // Verify the project path exists
    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {}", project_path.display());
    }

    if !project_path.is_dir() {
        anyhow::bail!(
            "Project path is not a directory: {}",
            project_path.display()
        );
    }

    tracing::info!(
        "Starting LSP experiment with project: {} (Language: {}, Extensions: {})",
        project_path.display(),
        language,
        language.extensions()
    );

    // Find all files of the specified language in the project
    tracing::info!("Scanning for {} files in project...", language);
    let finder = FileFinder::new();
    let matching_files = finder.find_language_files(&project_path, language)?;

    tracing::info!("Found {} {} files:", matching_files.len(), language);
    for file in &matching_files {
        tracing::info!("  {}", file.display());
    }

    // Start LSP server for the language
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = start_lsp_server(language, &project_path)?;

    tracing::info!(
        "LSP server started successfully in: {}",
        lsp_server.working_dir.display()
    );

    // Get stdin and stdout for LSP communication
    let mut stdin = lsp_server
        .process
        .stdin
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
    let stdout = lsp_server
        .process
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
    let mut reader = BufReader::new(stdout);

    // Send Initialize request
    tracing::info!("Sending initialize request...");
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

    send_request(&mut stdin, 1, Initialize::METHOD, &initialize_params)?;
    let _init_response = read_response(&mut reader)?;
    tracing::info!("Received initialize response");

    // Send initialized notification
    send_notification(&mut stdin, "initialized", &serde_json::json!({}))?;
    tracing::info!("Sent initialized notification");

    // Wait a bit for the server to be ready
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Request definition for ScrollOffset.swift, line 31, character 17
    let file_path = project_path.join("SignalUI/Appearance/SwiftUI/ScrollOffset.swift");
    let file_uri = format!("file://{}", file_path.display());

    // Read the file content
    let file_content = std::fs::read_to_string(&file_path)?;

    // Send textDocument/didOpen notification
    tracing::info!("Opening document: {}", file_path.display());
    send_notification(
        &mut stdin,
        "textDocument/didOpen",
        &serde_json::json!({
            "textDocument": {
                "uri": file_uri,
                "languageId": "swift",
                "version": 1,
                "text": file_content
            }
        }),
    )?;

    tracing::info!("Requesting definition at {}:31:17", file_path.display());

    let definition_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
            position: Position {
                line: 30, // LSP uses 0-based line numbers
                character: 17,
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    send_request(&mut stdin, 2, GotoDefinition::METHOD, &definition_params)?;

    tracing::info!("Waiting for definition response...");
    let definition_response = read_response_with_id(&mut reader, 2)?;

    // Parse the response
    if let Some(result) = definition_response.get("result") {
        if result.is_null() {
            tracing::warn!("No definition found at the specified location");
        } else {
            match from_value::<GotoDefinitionResponse>(result.clone()) {
                Ok(response) => {
                    tracing::info!("Definition response: {:#?}", response);
                }
                Err(e) => {
                    tracing::error!("Failed to parse definition response: {}", e);
                    tracing::debug!("Raw response: {:#?}", result);
                }
            }
        }
    }

    // Keep the server running for a bit
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Stop the server
    tracing::info!("Stopping LSP server...");
    if let Err(e) = lsp_server.stop() {
        tracing::error!("Error stopping LSP server: {}", e);
    }

    Ok(())
}

fn send_request<T: serde::Serialize>(
    stdin: &mut dyn Write,
    id: u64,
    method: &str,
    params: &T,
) -> Result<()> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": to_value(params)?
    });

    let request_str = serde_json::to_string(&request)?;
    let message = format!(
        "Content-Length: {}\r\n\r\n{}",
        request_str.len(),
        request_str
    );

    tracing::debug!("Sending request: {}", request_str);
    stdin.write_all(message.as_bytes())?;
    stdin.flush()?;

    Ok(())
}

fn send_notification<T: serde::Serialize>(
    stdin: &mut dyn Write,
    method: &str,
    params: &T,
) -> Result<()> {
    let notification = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": to_value(params)?
    });

    let notification_str = serde_json::to_string(&notification)?;
    let message = format!(
        "Content-Length: {}\r\n\r\n{}",
        notification_str.len(),
        notification_str
    );

    tracing::debug!("Sending notification: {}", notification_str);
    stdin.write_all(message.as_bytes())?;
    stdin.flush()?;

    Ok(())
}

fn read_response(reader: &mut BufReader<std::process::ChildStdout>) -> Result<serde_json::Value> {
    // Read headers
    let mut content_length = 0;
    loop {
        let mut header = String::new();
        reader.read_line(&mut header)?;

        if header == "\r\n" {
            break;
        }

        if let Some(length_str) = header.strip_prefix("Content-Length: ") {
            content_length = length_str.trim().parse()?;
        }
    }

    // Read content
    let mut buffer = vec![0; content_length];
    std::io::Read::read_exact(reader, &mut buffer)?;

    let response_str = String::from_utf8(buffer)?;
    tracing::debug!("Received message: {}", response_str);

    let response: serde_json::Value = serde_json::from_str(&response_str)?;
    Ok(response)
}

fn read_response_with_id(
    reader: &mut BufReader<std::process::ChildStdout>,
    expected_id: u64,
) -> Result<serde_json::Value> {
    // Keep reading messages until we find the response with the matching ID
    loop {
        let message = read_response(reader)?;

        // Check if this is a notification (no id field) or response
        if let Some(id) = message.get("id") {
            if id.as_u64() == Some(expected_id) {
                return Ok(message);
            } else {
                tracing::debug!("Received response with different ID: {:?}", id);
            }
        } else {
            // This is a notification or other message without an ID
            if let Some(method) = message.get("method") {
                tracing::debug!("Received notification: {}", method);
            }
        }
    }
}
