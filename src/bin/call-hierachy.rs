//! Example demonstrating how to find all symbols in files of a project.
//!
//! Usage: cargo run --bin call-hierachy -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::{
    CallHierarchyIncomingCallsParams, CallHierarchyOutgoingCall, CallHierarchyOutgoingCallsParams,
    CallHierarchyPrepareParams, Range, TextDocumentPositionParams,
    request::{CallHierarchyIncomingCalls, CallHierarchyOutgoingCalls, CallHierarchyPrepare},
};
use lsp_types::{DocumentSymbol, SymbolKind};
use std::{os::unix::thread, path::Path, time::Duration};
use tree_sitter_lsp_experiment::lsp::text_document_identifier_from_path;
use tree_sitter_lsp_experiment::{
    Args, FileSearchConfig, GoLang, Language, LspServer, PythonLang, RustLang, SwiftLang,
    TypeScriptLang,
};
use tree_sitter_lsp_experiment::{location::highlight_range, lsp};

fn mk_outgoing_call_result(call: &CallHierarchyOutgoingCall) -> Option<OutgoingCallResult> {
    Some(OutgoingCallResult {
        to_name: call.to.name.clone(),
        to_kind: call.to.kind,
        to_range: call.to.range,
        to_selection_range: call.to.selection_range,
        from_ranges: *call.from_ranges.first()?,
    })
}

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

            let before_prepare = std::time::Instant::now();
            // Prepare call hierarchy at the symbol's position

            // Only enable retries for the first two symbols, as the LSP server
            // might not have finished loading the file yet.
            let enable_retries = i < 2;
            let Some(item) =
                prepare_call_hierarchy(&mut lsp_server, &absolute_path, symbol, enable_retries)?
            else {
                println!(
                    "  No call hierarchy items found after {:?} (including retries)",
                    before_prepare.elapsed()
                );
                continue;
            };

            // let prepare_params = CallHierarchyPrepareParams {
            //     text_document_position_params: TextDocumentPositionParams {
            //         text_document: text_document_identifier_from_path(&absolute_path)?,
            //         position: symbol.selection_range.start,
            //     },
            //     work_done_progress_params: Default::default(),
            // };
            // let prepare_response = lsp_server.request::<CallHierarchyPrepare>(prepare_params);
            // let prepare_elapsed = before_prepare.elapsed();

            // let call_hierarchy_items = match prepare_response {
            //     Ok(Some(items)) => items,
            //     Ok(None) => {
            //         println!("  No call hierarchy available ({:?})", prepare_elapsed);
            //         continue;
            //     }
            //     Err(e) => {
            //         tracing::warn!(
            //             "Failed to prepare call hierarchy ({:?}): {}",
            //             prepare_elapsed,
            //             e
            //         );
            //         continue;
            //     }
            // };

            // if call_hierarchy_items.is_empty() {
            //     println!("  No call hierarchy items found ({:?})", prepare_elapsed);
            //     continue;
            // }

            // println!("  Prepared call hierarchy ({:?})", prepare_elapsed);
            // let item = &call_hierarchy_items[0];

            // let before_incoming = std::time::Instant::now();
            // // Get incoming calls
            // let incoming_params = CallHierarchyIncomingCallsParams {
            //     item: item.clone(),
            //     work_done_progress_params: Default::default(),
            //     partial_result_params: Default::default(),
            // };
            // match lsp_server.request::<CallHierarchyIncomingCalls>(incoming_params) {
            //     Ok(Some(incoming)) => {
            //         println!(
            //             "  Incoming calls after {:?} ({}):",
            //             before_incoming.elapsed(),
            //             incoming.len()
            //         );
            //         total_incoming_calls += incoming.len();
            //         for call in incoming.iter().take(10) {
            //             println!(
            //                 "    <- {} ({}:{})",
            //                 call.from.name,
            //                 call.from.uri.path(),
            //                 call.from.selection_range.start.line + 1
            //             );
            //         }
            //         if incoming.len() > 10 {
            //             println!("    ... and {} more", incoming.len() - 10);
            //         }
            //     }
            //     Ok(None) => println!("  Incoming calls: 0"),
            //     Err(e) => tracing::warn!("  Failed to get incoming calls: {}", e),
            // }

            let before_outgoing = std::time::Instant::now();
            // Get outgoing calls
            let outgoing_params = CallHierarchyOutgoingCallsParams {
                item: item.clone(),
                work_done_progress_params: Default::default(),
                partial_result_params: Default::default(),
            };

            match lsp_server.request::<CallHierarchyOutgoingCalls>(outgoing_params) {
                Ok(Some(outgoing)) => {
                    println!(
                        "  Outgoing calls after {:?} ({}):",
                        before_outgoing.elapsed(),
                        outgoing.len()
                    );
                    let _results = outgoing
                        .iter()
                        .filter_map(mk_outgoing_call_result)
                        .collect::<Vec<_>>();
                    total_calls += outgoing.len();
                    for call in outgoing.iter().take(10) {
                        // Get the line number and source code where the call is made from
                        let from_line_str = match call.from_ranges.first() {
                            Some(range) => {
                                highlight_range(&file_lines, *range);
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
                    if outgoing.len() > 10 {
                        println!("    ... and {} more", outgoing.len() - 10);
                    }
                }
                Ok(None) => println!("  Outgoing calls: 0"),
                Err(e) => tracing::warn!("  Failed to get outgoing calls: {}", e),
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
