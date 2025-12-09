//! Example demonstrating how to find all references to functions/methods in a project.
//!
//! Usage: cargo run --bin find_all_references -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::{
    ReferenceContext, ReferenceParams, SymbolKind, TextDocumentPositionParams, request::References,
};
use std::path::PathBuf;
use std::{env, path::Path};
use tree_sitter_lsp_experiment::lsp::text_document_identifier_from_path;
use tree_sitter_lsp_experiment::{
    FileSearchConfig, GoLang, Language, LspServer, PythonLang, RustLang, SwiftLang, TypeScriptLang,
};

fn process_files<L: Language>(
    language: L,
    project_path: &Path,
    config: &FileSearchConfig,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let mut total_symbols = 0;
    let mut total_references = 0;

    // Find all matching files
    let matching_files = config.find_language_files(project_path, language)?;

    if matching_files.is_empty() {
        println!("No matching files found in {}", project_path.display());
        return Ok(());
    }

    println!("Found {} matching files", matching_files.len());

    // Start and initialize LSP server
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = LspServer::start_and_init(language, project_path.to_path_buf())?;

    // Give LSP server time to start indexing
    tracing::info!("Giving LSP server time to start indexing...");
    std::thread::sleep(std::time::Duration::from_secs(1));

    // Process each file
    for (index, file_path) in matching_files.iter().enumerate() {
        // Skip if file name contains spaces (can cause URI issues)
        if file_path.display().to_string().contains(' ') {
            tracing::debug!("Skipping file with spaces in path: {}", file_path.display());
            continue;
        }

        println!("\n{}", "=".repeat(80));
        println!(
            "[{}/{}] Processing: {}",
            index + 1,
            matching_files.len(),
            file_path.display()
        );
        println!("{}", "=".repeat(80));

        // Get absolute path
        let absolute_path = match file_path.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                tracing::warn!("Failed to canonicalize path {}: {}", file_path.display(), e);
                continue;
            }
        };

        // Read file content
        let file_content = match std::fs::read_to_string(&absolute_path) {
            Ok(content) => content,
            Err(e) => {
                tracing::warn!("Failed to read file {}: {}", absolute_path.display(), e);
                continue;
            }
        };

        // Open the document in the LSP server
        if let Err(e) = lsp_server.open_file(&absolute_path, &file_content) {
            tracing::warn!("Failed to open document {}: {}", absolute_path.display(), e);
            continue;
        }

        // Request document symbols
        let before_symbols = std::time::Instant::now();
        let (symbols, is_flat) = lsp_server.get_document_symbols(&absolute_path)?;
        println!(
            "Found {} symbols ({}) in {:.2?}",
            symbols.len(),
            if is_flat { "flat" } else { "nested" },
            before_symbols.elapsed()
        );

        // Recursively collect all callable symbols (functions/methods)
        fn collect_callable_symbols<'a>(
            symbols: &'a [lsp_types::DocumentSymbol],
            result: &mut Vec<&'a lsp_types::DocumentSymbol>,
        ) {
            for symbol in symbols {
                if matches!(
                    symbol.kind,
                    SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR
                ) {
                    result.push(symbol);
                }
                // Recursively process children
                if let Some(ref children) = symbol.children {
                    collect_callable_symbols(children, result);
                }
            }
        }

        let mut callable_symbols = Vec::new();
        collect_callable_symbols(&symbols, &mut callable_symbols);

        println!(
            "\nFound {} callable symbols (functions/methods/constructors)",
            callable_symbols.len()
        );

        total_symbols += callable_symbols.len();

        // Find references for each callable symbol
        for (i, symbol) in callable_symbols.iter().enumerate() {
            println!(
                "\n[{}/{}] Analyzing references for: {}",
                i + 1,
                callable_symbols.len(),
                symbol.name
            );

            // Request references at the symbol's position with exponential backoff
            let reference_params = ReferenceParams {
                text_document_position: TextDocumentPositionParams {
                    text_document: text_document_identifier_from_path(&absolute_path)?,
                    position: symbol.selection_range.start,
                },
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
                context: ReferenceContext {
                    include_declaration: true,
                },
            };

            // Exponential backoff only for the first symbol in each file
            // After the first symbol, the LSP has indexed the file and subsequent queries are fast
            // Delays: 0ms, 50ms, 250ms (only for first symbol)
            let is_first_symbol = i == 0;
            // let max_attempts = 1;
            let max_attempts = if is_first_symbol { 3 } else { 1 };
            let mut found_references = false;
            let backoff_start = std::time::Instant::now();

            for attempt in 0..max_attempts {
                if attempt > 0 {
                    let delay_ms = if attempt == 1 { 50 } else { 250 };
                    tracing::info!(
                        "    Retry attempt {} after {}ms delay for '{}' (first symbol in file)",
                        attempt + 1,
                        delay_ms,
                        symbol.name
                    );
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }

                let request_start = std::time::Instant::now();
                match lsp_server.request::<References>(reference_params.clone()) {
                    Ok(Some(locations)) if locations.len() > 0 => {
                        let request_time = request_start.elapsed();
                        tracing::info!(
                            "    Request took {:.2?}, found {} references on attempt {}",
                            request_time,
                            locations.len(),
                            attempt + 1
                        );
                        println!("  Found {} references:", locations.len());
                        total_references += locations.len();

                        for (j, location) in locations.iter().enumerate().take(10) {
                            let file_path = location.uri.path();
                            let line = location.range.start.line + 1;
                            let char = location.range.start.character;
                            println!("    {}. {}:{}:{}", j + 1, file_path, line, char);
                        }

                        if locations.len() > 10 {
                            println!("    ... and {} more", locations.len() - 10);
                        }
                        found_references = true;
                        break;
                    }
                    Ok(Some(_)) | Ok(None) => {
                        let request_time = request_start.elapsed();
                        tracing::info!(
                            "    Request took {:.2?}, no references found on attempt {}",
                            request_time,
                            attempt + 1
                        );
                        // No references yet, will retry if attempts remain
                        if attempt == max_attempts - 1 {
                            println!("  No references found");
                        }
                    }
                    Err(e) => {
                        tracing::warn!("  Failed to get references: {}", e);
                        break;
                    }
                }
            }

            let total_backoff_time = backoff_start.elapsed();
            if found_references {
                tracing::info!("    Total time with backoff: {:.2?}", total_backoff_time);
            } else if max_attempts > 1 {
                tracing::info!(
                    "    No references found after {} attempts (total time: {:.2?})",
                    max_attempts,
                    total_backoff_time
                );
            }
        }
        lsp_server.close_file(&absolute_path)?;
    }

    let elapsed = start_time.elapsed();
    let symbols_per_sec = total_symbols as f64 / elapsed.as_secs_f64();
    println!("\n{}", "=".repeat(80));
    println!(
        "Summary: Analyzed {} symbols, found {} total references in {:.2?} ({:.2} symbols/sec)",
        total_symbols, total_references, elapsed, symbols_per_sec
    );

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

    println!(
        "Finding all references to functions/methods in {}",
        project_path.display()
    );

    // Process files based on language
    match language {
        "rust" => process_files(RustLang, &project_path, &config)?,
        "python" => process_files(PythonLang, &project_path, &config)?,
        "typescript" => process_files(TypeScriptLang, &project_path, &config)?,
        "go" => process_files(GoLang, &project_path, &config)?,
        "swift" => process_files(SwiftLang, &project_path, &config)?,
        lang => anyhow::bail!("Unsupported language: {}.", lang),
    };

    Ok(())
}
