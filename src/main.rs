use anyhow::Result;
use lsp_types::{
    DidOpenTextDocumentParams, GotoDefinitionParams, InitializeParams, InitializedParams, Position,
    TextDocumentIdentifier, TextDocumentItem, TextDocumentPositionParams, WorkspaceFolder,
    notification::{DidOpenTextDocument, Initialized},
    request::{GotoDefinition, Initialize},
};
use std::env;
use std::path::PathBuf;
use tree_sitter_lsp_experiment::{
    FileSearchConfig, GoLang, Language, LspServer, LspServerConfig, PythonLang, RustLang,
    SwiftLang, TypeScriptLang,
};

fn start<L: Language + Copy>(language: L, project_path: PathBuf) -> Result<()> {
    tracing::info!(
        "Starting LSP experiment with project: {} (Language: {}, Extensions: {})",
        project_path.display(),
        language,
        language.extensions()
    );

    // Find all files of the specified language in the project
    tracing::info!("Scanning for {} files in project...", language);
    let config = FileSearchConfig::default();
    let matching_files = config.find_language_files(&project_path, language)?;

    tracing::info!("Found {} {} files:", matching_files.len(), language);
    for file in &matching_files {
        tracing::info!("  {}", file.display());
    }

    // Start LSP server for the language
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server =
        LspServer::<L>::start(language, project_path.clone(), LspServerConfig::default())?;

    tracing::info!(
        "LSP server started successfully in: {}",
        lsp_server.working_dir.display()
    );

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

    let _init_response = lsp_server.request::<Initialize>(initialize_params)?;
    tracing::info!("Received initialize response");

    // Send initialized notification
    lsp_server.send_notification::<Initialized>(InitializedParams {})?;
    tracing::info!("Sent initialized notification");

    // Request definition for ScrollOffset.swift, line 31, character 17
    // let file_path = project_path.join("SignalUI/Appearance/SwiftUI/ScrollOffset.swift");
    let file_path = project_path.join("src/main.rs");
    let file_uri = format!("file://{}", file_path.display());

    // Read the file content
    let file_content = std::fs::read_to_string(&file_path)?;

    // Send textDocument/didOpen notification
    tracing::info!("Opening document: {}", &file_path.display());
    lsp_server.send_notification::<DidOpenTextDocument>(DidOpenTextDocumentParams {
        text_document: TextDocumentItem {
            uri: file_uri.parse()?,
            language_id: language.to_string().to_lowercase(),
            version: 1,
            text: file_content.clone(),
        },
    })?;

    // Give rust-analyzer time to index the project
    // rust-analyzer needs to load and analyze the project which can take a moment
    tracing::info!("Waiting for LSP server to index the project...");
    std::thread::sleep(std::time::Duration::from_secs(3));

    tracing::info!(
        "Requesting definition at {}:8:18 (the 'add' function call)",
        file_path.display()
    );

    let definition_params = GotoDefinitionParams {
        text_document_position_params: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
            // Line 7 (0-indexed) is: "let result = add(x, y);"
            // Character 17 is on the 'a' in 'add'
            position: Position {
                line: 7,       // LSP uses 0-based line numbers
                character: 17, // Position on the function name 'add'
            },
        },
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };

    tracing::info!("Requesting definition...");

    // Retry the request if rust-analyzer reports "content modified"
    let definition_response = lsp_server.request::<GotoDefinition>(definition_params.clone())?;
    // {
    //     Ok(response) => response,
    //     Err(e) if e.to_string().contains("content modified") => {
    //         tracing::debug!("Got 'content modified' error, retrying in 500ms...");
    //         panic!("Got 'content modified' error, retrying in 500ms...");
    //     }
    //     Err(e) => return Err(e),
    // };

    if let Some(response) = definition_response {
        tracing::info!("Definition response: {:#?}", response);
    } else {
        tracing::warn!("No definition found at the specified location");
    }

    // Stop the server
    tracing::info!("Stopping LSP server...");
    if let Err(e) = lsp_server.stop() {
        tracing::error!("Error stopping LSP server: {}", e);
    }

    Ok(())
}

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
    let mut language: Option<&String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--language" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --language requires a value");
                    std::process::exit(1);
                }
                language = Some(&args[i + 1]);
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

    // Verify that the project path exists
    if !project_path.exists() {
        anyhow::bail!("Project path does not exist: {}", project_path.display());
    }

    if !project_path.is_dir() {
        anyhow::bail!(
            "Project path is not a directory: {}",
            project_path.display()
        );
    }

    match language.as_str() {
        "rust" => start(RustLang, project_path)?,
        "python" => start(PythonLang, project_path)?,
        "typescript" => start(TypeScriptLang, project_path)?,
        "go" => start(GoLang, project_path)?,
        "swift" => start(SwiftLang, project_path)?,
        lang => anyhow::bail!("Unsupported language: {}.", lang),
    };

    Ok(())
}
