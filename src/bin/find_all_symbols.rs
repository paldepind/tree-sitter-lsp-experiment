//! Example demonstrating how to find all symbols in files of a project.
//!
//! Usage: cargo run --bin find_all_symbols -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::{DocumentSymbolParams, TextDocumentIdentifier, request::DocumentSymbolRequest};
use std::env;
use std::path::PathBuf;
use tree_sitter_lsp_experiment::{
    FileSearchConfig, GoLang, Language, LspServer, LspServerConfig, PythonLang, RustLang,
    SwiftLang, TypeScriptLang,
};

fn process_files<L: Language>(
    language: L,
    project_path: &PathBuf,
    config: &FileSearchConfig,
) -> Result<()> {
    // Find all matching files
    let matching_files = config.find_language_files(project_path, language)?;

    if matching_files.is_empty() {
        println!("No matching files found in {}", project_path.display());
        return Ok(());
    }

    println!("Found {} matching files", matching_files.len());

    // Start and initialize LSP server
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = LspServer::start_and_init(
        language,
        project_path.to_path_buf(),
        LspServerConfig::default(),
    )?;

    // Process each file
    for (index, file_path) in matching_files.iter().enumerate() {
        println!("\n{}", "=".repeat(80));
        println!(
            "[{}/{}] Processing: {}",
            index + 1,
            matching_files.len(),
            file_path.display()
        );
        println!("{}", "=".repeat(80));

        // Read file content
        let file_content = match std::fs::read_to_string(file_path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!("Failed to read file {}: {}", file_path.display(), e);
                continue;
            }
        };

        // Open the document in the LSP server
        if let Err(e) = lsp_server.open_file(file_path, &file_content) {
            tracing::warn!("Failed to open document {}: {}", file_path.display(), e);
            continue;
        }

        // Request document symbols
        let file_uri = format!("file://{}", file_path.display());
        let params = DocumentSymbolParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
        };

        match lsp_server.request::<DocumentSymbolRequest>(params) {
            Ok(Some(response)) => {
                // Pretty print the response
                let json = serde_json::to_string_pretty(&response)?;
                println!("{}", json);
            }
            Ok(None) => {
                println!("No symbols found in this file");
            }
            Err(e) => {
                tracing::warn!("Failed to get symbols for {}: {}", file_path.display(), e);
            }
        }

        // Close the document in the LSP server
        if let Err(e) = lsp_server.close_file(file_path) {
            tracing::warn!("Failed to close document {}: {}", file_path.display(), e);
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 4 {
        eprintln!(
            "Usage: {} <project_path> --language <language> [--include <glob_pattern>] [--exclude <glob_pattern>]",
            args[0]
        );
        eprintln!("Supported languages: rust, python, typescript, go, swift");
        std::process::exit(1);
    }

    let project_path = PathBuf::from(&args[1]);

    // Parse arguments
    let mut language = None;
    let mut include_pattern = None;
    let mut exclude_pattern = None;
    let mut i = 2;

    while i < args.len() {
        match args[i].as_str() {
            "--language" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --language requires a value");
                    std::process::exit(1);
                }
                language = Some(args[i + 1].as_str());
                i += 2;
            }
            "--include" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --include requires a value");
                    std::process::exit(1);
                }
                include_pattern = Some(args[i + 1].clone());
                i += 2;
            }
            "--exclude" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --exclude requires a value");
                    std::process::exit(1);
                }
                exclude_pattern = Some(args[i + 1].clone());
                i += 2;
            }
            _ => {
                eprintln!("Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
    }

    let language = language.unwrap_or_else(|| {
        eprintln!("Error: --language is required");
        std::process::exit(1);
    });

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

    // Create file search config
    let mut config = FileSearchConfig::default();
    if let Some(pattern) = include_pattern {
        let glob_pattern = glob::Pattern::new(&pattern)
            .map_err(|e| anyhow::anyhow!("Invalid include glob pattern '{}': {}", pattern, e))?;
        config.include_glob = Some(glob_pattern);
        println!("Using include pattern: {}", pattern);
    }
    if let Some(pattern) = exclude_pattern {
        let glob_pattern = glob::Pattern::new(&pattern)
            .map_err(|e| anyhow::anyhow!("Invalid exclude glob pattern '{}': {}", pattern, e))?;
        config.exclude_glob = Some(glob_pattern);
        println!("Using exclude pattern: {}", pattern);
    }

    println!("Finding all symbols in files in {}", project_path.display());

    // Initialize performance timer
    let start_time = std::time::Instant::now();

    // Process files based on language
    match language {
        "rust" => process_files(RustLang, &project_path, &config)?,
        "python" => process_files(PythonLang, &project_path, &config)?,
        "typescript" => process_files(TypeScriptLang, &project_path, &config)?,
        "go" => process_files(GoLang, &project_path, &config)?,
        "swift" => process_files(SwiftLang, &project_path, &config)?,
        lang => anyhow::bail!("Unsupported language: {}.", lang),
    };

    let elapsed = start_time.elapsed();
    println!("\n{}", "=".repeat(80));
    println!("Completed in {:.2?}", elapsed);

    Ok(())
}
