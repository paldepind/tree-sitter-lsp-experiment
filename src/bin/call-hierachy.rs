//! Example demonstrating how to find all symbols in files of a project.
//!
//! Usage: cargo run --bin call-hierachy -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCallsParams, CallHierarchyPrepareParams,
    TextDocumentPositionParams,
    request::{CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare},
};
use lsp_types::{DocumentSymbol, SymbolKind};
use std::{path::Path, time::Duration};
use tree_sitter_lsp_experiment::location::print_highlighted_range;
use tree_sitter_lsp_experiment::{
    Args, FileSearchConfig, GoLang, Language, LspServer, PythonLang, RustLang, SwiftLang,
    TypeScriptLang,
};
use tree_sitter_lsp_experiment::{
    lsp::text_document_identifier_from_path, parser::parse_file_content,
};

// fn mk_outgoing_call_result(call: &CallHierarchyOutgoingCall) -> Option<OutgoingCallResult> {
//     Some(OutgoingCallResult {
//         to_name: call.to.name.clone(),
//         to_kind: call.to.kind,
//         to_range: call.to.range,
//         to_selection_range: call.to.selection_range,
//         from_ranges: *call.from_ranges.first()?,
//     })
// }

fn extract_call_hierachy<L: Language>(
    language: L,
    project_path: &Path,
    config: &FileSearchConfig,
) -> Result<()> {
    // Find all matching files
    let matching_files = config.find_language_files(project_path, language)?;

    if matching_files.is_empty() {
        println!("No matching files found in {}", project_path.display());
        return Ok(());
    }

    println!("Found {} matching files", matching_files.len());
    println!("{:?}", matching_files);

    extract_call_hierachy_for_files(language, project_path, &matching_files)
}

// Recursively collect all callable symbols (functions/methods) including nested ones
// Note: for flat symbols, there won't be any children
fn collect_symbols_with_calls<'a>(
    symbols: &'a [lsp_types::DocumentSymbol],
    result: &mut Vec<&'a lsp_types::DocumentSymbol>,
) {
    for symbol in symbols {
        if matches!(
            symbol.kind,
            SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR // | SymbolKind::PROPERTY
                                                                                // | SymbolKind::FIELD
                                                                                // | SymbolKind::ENUM_MEMBER
        ) {
            result.push(symbol);
        }
        // Recursively process children, skip interfaces as these rarely contain calls
        if !matches!(symbol.kind, SymbolKind::INTERFACE)
            && let Some(ref children) = symbol.children
        {
            collect_symbols_with_calls(children, result);
        }
    }
}

fn prepare_call_hierarchy(
    lsp_server: &mut LspServer<impl Language>,
    absolute_path: &Path,
    symbol: &DocumentSymbol,
    enable_retries: bool,
) -> Result<Option<lsp_types::CallHierarchyItem>> {
    for retries in 0..6 {
        if retries > 0 {
            std::thread::sleep(std::time::Duration::from_millis(100u64 * retries));
        }
        let before_prepare = std::time::Instant::now();

        let prepare_params = CallHierarchyPrepareParams {
            text_document_position_params: TextDocumentPositionParams {
                text_document: text_document_identifier_from_path(absolute_path)?,
                position: symbol.selection_range.start,
            },
            work_done_progress_params: Default::default(),
        };
        let prepare_response = lsp_server.request::<CallHierarchyPrepare>(prepare_params);
        let prepare_elapsed = before_prepare.elapsed();

        match prepare_response {
            Ok(Some(items)) => match items.into_iter().next() {
                Some(item) => {
                    println!("  Prepared call hierarchy ({:?})", prepare_elapsed);
                    return Ok(Some(item));
                }
                None => {
                    println!("  No call hierarchy items found ({:?})", prepare_elapsed);
                }
            },
            Ok(None) => {
                println!("  No call hierarchy available ({:?})", prepare_elapsed);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to prepare call hierarchy ({:?}): {}",
                    prepare_elapsed,
                    e
                );
                return Err(e);
            }
        };
        if !enable_retries {
            return Ok(None);
        }
    }
    Ok(None)
}

struct CallHierarchyResult {
    incoming: Vec<lsp_types::CallHierarchyIncomingCall>,
    outgoing: Vec<lsp_types::CallHierarchyOutgoingCall>,
}

/// Prepares call hierarchy and fetches both incoming and outgoing calls for a symbol
fn get_call_hierarchy(
    lsp_server: &mut LspServer<impl Language>,
    absolute_path: &Path,
    symbol: &DocumentSymbol,
    enable_retries: bool,
) -> Result<Option<CallHierarchyResult>> {
    let before_prepare = std::time::Instant::now();

    // Prepare call hierarchy
    let Some(item) = prepare_call_hierarchy(lsp_server, absolute_path, symbol, enable_retries)?
    else {
        println!(
            "  No call hierarchy items found after {:?} (including retries)",
            before_prepare.elapsed()
        );
        return Ok(None);
    };

    let before_incoming = std::time::Instant::now();
    // Get incoming calls
    let incoming_params = CallHierarchyIncomingCallsParams {
        item: item.clone(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let incoming = match lsp_server.request::<CallHierarchyIncomingCalls>(incoming_params) {
        Ok(Some(incoming)) => {
            println!(
                "  Incoming calls after {:?} ({}):",
                before_incoming.elapsed(),
                incoming.len()
            );
            incoming
        }
        Ok(None) => {
            println!("  Incoming calls: 0");
            Vec::new()
        }
        Err(e) => {
            tracing::warn!("  Failed to get incoming calls: {}", e);
            Vec::new()
        }
    };

    let before_outgoing = std::time::Instant::now();
    // Get outgoing calls
    let outgoing_params = CallHierarchyOutgoingCallsParams {
        item: item.clone(),
        work_done_progress_params: Default::default(),
        partial_result_params: Default::default(),
    };
    let outgoing = match lsp_server.request::<CallHierarchyOutgoingCalls>(outgoing_params) {
        Ok(Some(outgoing)) => {
            println!(
                "  Outgoing calls after {:?} ({}):",
                before_outgoing.elapsed(),
                outgoing.len()
            );
            outgoing
        }
        Ok(None) => {
            println!("  Outgoing calls: 0");
            Vec::new()
        }
        Err(e) => {
            tracing::warn!("  Failed to get outgoing calls: {}", e);
            Vec::new()
        }
    };

    Ok(Some(CallHierarchyResult { incoming, outgoing }))
}

fn extract_call_hierachy_for_files<L: Language>(
    language: L,
    project_path: &Path,
    files: &[std::path::PathBuf],
) -> Result<()> {
    let mut total_calls = 0;
    let mut total_incoming_calls = 0;
    let mut total_symbols = 0;

    // Start and initialize LSP server
    tracing::info!("Starting LSP server for {}...", language);
    let mut lsp_server = LspServer::start_and_init(language, project_path.to_path_buf())?;

    let mut durations = Vec::<(&str, Duration)>::new();

    // NOTE: It seems that for some LSP servers, giving them a bit of time to
    // start makes it possible for them to resolve more call hierarchy requests.
    std::thread::sleep(std::time::Duration::from_millis(1000));

    let start_time = std::time::Instant::now();
    // Process each file
    for (index, file_path) in files.iter().enumerate() {
        // Skip if file name contains spaces
        if file_path.display().to_string().contains(' ') {
            println!(
                "\n[Skipping {}/{}] File name contains spaces: {}",
                index + 1,
                files.len(),
                file_path.display()
            );
            continue;
        }
        println!("\n{}", "=".repeat(80));
        println!(
            "[{}/{}] Processing: {}",
            index + 1,
            files.len(),
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

        // Split file content into lines for later source code display
        let file_lines: Vec<&str> = file_content.lines().collect();

        // Open the document in the LSP server
        if let Err(e) = lsp_server.open_file(&absolute_path, &file_content) {
            tracing::warn!("Failed to open document {}: {}", absolute_path.display(), e);
            continue;
        }

        // Request document symbols
        let before_symbols = std::time::Instant::now();
        let (symbols, is_flat) = lsp_server.get_document_symbols(&absolute_path)?;
        let symbols_elapsed = before_symbols.elapsed();

        durations.push((file_path.to_str().unwrap_or(""), symbols_elapsed));

        println!(
            "Found {} symbols ({}) in {:.2?}",
            symbols.len(),
            if is_flat { "flat" } else { "nested" },
            symbols_elapsed
        );
        let before_parse = std::time::Instant::now();
        let _ = parse_file_content(&file_content, language)?;
        println!("Parsed file content in {:.2?}", before_parse.elapsed());

        let mut symbols_with_calls = Vec::new();
        collect_symbols_with_calls(&symbols, &mut symbols_with_calls);

        println!(
            "\nFound {} callable symbols (functions/methods)",
            symbols_with_calls.len()
        );
        total_symbols += symbols_with_calls.len();

        // Get call hierarchy information for each callable symbol
        for (i, symbol) in symbols_with_calls.iter().enumerate() {
            println!(
                "\n[{}/{}] [{}/{}] Analyzing calls for: {}",
                index + 1,
                files.len(),
                i + 1,
                symbols_with_calls.len(),
                symbol.name
            );

            // Check if LSP server is still alive before trying to use it
            if !lsp_server.is_alive() {
                println!("  Skipping - LSP server has terminated");
                tracing::warn!("LSP server is no longer running, stopping file processing");
                return Ok(());
            }

            // Only enable retries for the first two symbols, as the LSP server
            // might not have finished loading the file yet.
            let enable_retries = i < 2;

            let result =
                match get_call_hierarchy(&mut lsp_server, &absolute_path, symbol, enable_retries) {
                    Ok(Some(r)) => r,
                    Ok(None) => {
                        println!("  No call hierarchy available");
                        continue;
                    }
                    Err(e) => {
                        println!("  Error: {}", e);
                        tracing::warn!("Failed to get call hierarchy for {}: {}", symbol.name, e);

                        // If the server has died, stop trying to process more symbols
                        if !lsp_server.is_alive() {
                            tracing::warn!("LSP server died, stopping file processing");
                            break;
                        }
                        continue;
                    }
                };

            // Display incoming calls
            total_incoming_calls += result.incoming.len();
            for call in result.incoming.iter().take(10) {
                println!(
                    "    <- {} ({}:{})",
                    call.from.name,
                    call.from.uri.path(),
                    call.from.selection_range.start.line + 1
                );
            }
            if result.incoming.len() > 10 {
                println!("    ... and {} more", result.incoming.len() - 10);
            }

            // Display outgoing calls
            total_calls += result.outgoing.len();
            for call in result.outgoing.iter().take(10) {
                // Get the line number and source code where the call is made from
                let from_line_str = match call.from_ranges.first() {
                    Some(range) => {
                        print_highlighted_range(&file_lines, *range);
                        let line_num = range.start.line as usize;
                        format!("from line {}", line_num + 1)
                    }
                    None => {
                        panic!("wwwahhhhtt");
                        // String::from("from unknown line"),
                    }
                };

                println!(
                    "    -> {} ({}:{}) {}",
                    call.to.name,
                    call.to.uri.path(),
                    call.to.selection_range.start.line + 1,
                    from_line_str
                );
            }
            if result.outgoing.len() > 10 {
                println!("    ... and {} more", result.outgoing.len() - 10);
            }
        }

        // Close the document in the LSP server
        lsp_server.close_file(&absolute_path)?;
    }

    let elapsed = start_time.elapsed();
    let ops_per_sec = (total_calls + total_incoming_calls) as f64 / elapsed.as_secs_f64();

    println!(
        "Summary: {} calls with definitions and {} incoming calls found in {:.2?}, {:.2} calls/sec",
        total_calls, total_incoming_calls, elapsed, ops_per_sec
    );
    println!(
        "Symbols processed : {} {:.2} symbols/sec",
        total_symbols,
        total_symbols as f64 / elapsed.as_secs_f64()
    );
    println!(
        "Calls per request : {:.3}",
        total_calls as f64 / total_symbols as f64
    );
    durations.sort_by_key(|t| t.1);
    let total_durations: Duration = durations.iter().map(|(_, duration)| duration).sum();
    print!(
        "Total durations: {:.2?} n={}",
        total_durations,
        durations.len()
    );
    print!("All durations: {:?}", durations);

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
        "rust" => extract_call_hierachy(RustLang, &args.project_path, &config)?,
        "python" => extract_call_hierachy(PythonLang, &args.project_path, &config)?,
        "typescript" => extract_call_hierachy(TypeScriptLang, &args.project_path, &config)?,
        "go" => extract_call_hierachy(GoLang, &args.project_path, &config)?,
        "swift" => extract_call_hierachy(SwiftLang, &args.project_path, &config)?,
        _ => unreachable!(),
    }

    let elapsed = start_time.elapsed();
    println!("\n{}", "=".repeat(80));
    println!("Completed in {:.2?}", elapsed);

    Ok(())
}
