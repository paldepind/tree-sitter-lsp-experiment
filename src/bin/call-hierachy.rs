//! Example demonstrating how to find all symbols in files of a project.
//!
//! Usage: cargo run --bin call-hierachy -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::SymbolKind;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    TextDocumentPositionParams, Uri,
    request::{CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare},
};
use std::path::Path;
use tree_sitter_lsp_experiment::lsp::text_document_identifier_from_path;
use tree_sitter_lsp_experiment::{
    Args, FileSearchConfig, GoLang, Language, LspServer, PythonLang, RustLang, SwiftLang,
    TypeScriptLang, lsp::uri_from_path,
};

fn process_files<L: Language>(
    language: L,
    project_path: &Path,
    config: &FileSearchConfig,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let mut total_calls = 0;
    let mut total_incoming_calls = 0;

    // Find all matching files
    let matching_files = config.find_language_files(project_path, language)?;

    if matching_files.is_empty() {
        println!("No matching files found in {}", project_path.display());
        return Ok(());
    }

    println!("Found {} matching files", matching_files.len());
    println!("{:?}", matching_files);

    // Start and initialize LSP server
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = LspServer::start_and_init(language, project_path.to_path_buf())?;

    // NOTE: It seems that for some LSP servers, giving them a bit of time to
    // start makes it possible for them to resolve more call hierarchy requests.
    std::thread::sleep(std::time::Duration::from_millis(3000));

    // Process each file
    for (index, file_path) in matching_files.iter().enumerate() {
        // Skip if file name contains spaces
        if file_path.display().to_string().contains(' ')
        // .is_some_and(|name| name.to_string_lossy().contains(' '))
        {
            println!(
                "\n[Skipping {}/{}] File name contains spaces: {}",
                index + 1,
                matching_files.len(),
                file_path.display()
            );
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

        let file_uri: Uri = uri_from_path(file_path)?;

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
        let before_symbols = std::time::Instant::now();
        let (symbols, is_flat) = lsp_server.get_document_symbols(file_path)?;
        println!(
            "Found {} symbols ({}) in {:.2?}",
            symbols.len(),
            if is_flat { "flat" } else { "nested" },
            before_symbols.elapsed()
        );

        // Recursively collect all callable symbols (functions/methods) including nested ones
        // Note: for flat symbols, there won't be any children
        fn collect_symbols_with_calls<'a>(
            symbols: &'a [lsp_types::DocumentSymbol],
            result: &mut Vec<&'a lsp_types::DocumentSymbol>,
        ) {
            for symbol in symbols {
                if matches!(
                    symbol.kind,
                    SymbolKind::FUNCTION
                        | SymbolKind::METHOD
                        | SymbolKind::CONSTRUCTOR
                        | SymbolKind::PROPERTY
                        | SymbolKind::FIELD
                        | SymbolKind::ENUM_MEMBER
                ) {
                    result.push(symbol);
                }
                // Recursively process children
                if let Some(ref children) = symbol.children {
                    collect_symbols_with_calls(children, result);
                }
            }
        }

        let mut symbols_with_calls = Vec::new();
        collect_symbols_with_calls(&symbols, &mut symbols_with_calls);

        println!(
            "\nFound {} callable symbols (functions/methods)",
            symbols_with_calls.len()
        );

        // Get call hierarchy information for each callable symbol
        for (i, symbol) in symbols_with_calls.iter().enumerate() {
            println!(
                "\n[{}/{}] [{}/{}] Analyzing calls for: {}",
                index + 1,
                matching_files.len(),
                i + 1,
                symbols_with_calls.len(),
                symbol.name
            );

            // Prepare call hierarchy at the symbol's position
            let prepare_params = CallHierarchyPrepareParams {
                text_document_position_params: TextDocumentPositionParams {
                    text_document: text_document_identifier_from_path(file_path)?,
                    position: symbol.selection_range.start,
                },
                work_done_progress_params: Default::default(),
            };

            let call_hierarchy_items =
                match lsp_server.request::<CallHierarchyPrepare>(prepare_params) {
                    Ok(Some(items)) => items,
                    Ok(None) => {
                        println!("  No call hierarchy available");
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to prepare call hierarchy: {}", e);
                        continue;
                    }
                };

            if call_hierarchy_items.is_empty() {
                println!("  No call hierarchy items found");
                continue;
            }

            let item = &call_hierarchy_items[0];

            let before_incoming = std::time::Instant::now();
            // Get incoming calls
            let incoming_params = CallHierarchyIncomingCallsParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };
            match lsp_server.request::<CallHierarchyIncomingCalls>(incoming_params) {
                Ok(Some(incoming)) => {
                    println!(
                        "  Incoming calls after {:?} ({}):",
                        before_incoming.elapsed(),
                        incoming.len()
                    );
                    total_incoming_calls += incoming.len();
                    for call in incoming.iter().take(10) {
                        println!(
                            "    <- {} ({}:{})",
                            call.from.name,
                            call.from.uri.path(),
                            call.from.selection_range.start.line + 1
                        );
                    }
                    if incoming.len() > 10 {
                        println!("    ... and {} more", incoming.len() - 10);
                    }
                }
                Ok(None) => println!("  Incoming calls: 0"),
                Err(e) => tracing::warn!("  Failed to get incoming calls: {}", e),
            }

            let before_outgoing = std::time::Instant::now();
            // Get outgoing calls
            let outgoing_params = CallHierarchyOutgoingCallsParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };

            match lsp_server.request::<CallHierarchyOutgoingCalls>(outgoing_params) {
                Ok(Some(outgoing)) if !outgoing.is_empty() => {
                    println!(
                        "  Outgoing calls after {:?} ({}):",
                        before_outgoing.elapsed(),
                        outgoing.len()
                    );
                    total_calls += outgoing.len();
                    for call in outgoing.iter().take(10) {
                        println!(
                            "    -> {} ({}:{})",
                            call.to.name,
                            call.to.uri.path(),
                            call.to.selection_range.start.line + 1
                        );
                    }
                    if outgoing.len() > 10 {
                        println!("    ... and {} more", outgoing.len() - 10);
                    }
                }
                Ok(Some(_)) => println!("  Outgoing calls: 0"),
                Ok(None) => println!("  Outgoing calls: 0"),
                Err(e) => tracing::warn!("  Failed to get outgoing calls: {}", e),
            }
        }

        // Close the document in the LSP server
        lsp_server.close_file(file_path)?;
    }

    let elapsed = start_time.elapsed();
    let ops_per_sec = (total_calls + total_incoming_calls) as f64 / elapsed.as_secs_f64();
    println!(
        "Summary: {} calls with definitions and {} incoming calls found in {:.2?}, {:.2} ops/sec",
        total_calls, total_incoming_calls, elapsed, ops_per_sec
    );

    Ok(())
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse and validate command-line arguments
    let args = Args::parse_and_validate()?;
    let config = args.create_file_search_config()?;

    println!(
        "Finding all symbols in files in {}",
        args.project_path.display()
    );

    // Initialize performance timer
    let start_time = std::time::Instant::now();

    // Process files based on language
    match args.language.as_str() {
        "rust" => process_files(RustLang, &args.project_path, &config)?,
        "python" => process_files(PythonLang, &args.project_path, &config)?,
        "typescript" => process_files(TypeScriptLang, &args.project_path, &config)?,
        "go" => process_files(GoLang, &args.project_path, &config)?,
        "swift" => process_files(SwiftLang, &args.project_path, &config)?,
        _ => unreachable!(),
    }

    let elapsed = start_time.elapsed();
    println!("\n{}", "=".repeat(80));
    println!("Completed in {:.2?}", elapsed);

    Ok(())
}
