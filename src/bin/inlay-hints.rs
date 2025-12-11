//! Example demonstrating how to get inlay hints for files in a project.
//!
//! Usage: cargo run --bin inlay-hints -- <project_path> --language <language>

use anyhow::Result;
use lsp_types::{InlayHintParams, Range, TextDocumentIdentifier, WorkDoneProgressParams};
use std::path::Path;
use tree_sitter_lsp_experiment::{
    Args, FileSearchConfig, GoLang, Language, LspServer, PythonLang, RustLang, SwiftLang,
    TypeScriptLang,
};

fn process_files<L: Language>(
    language: L,
    project_path: &Path,
    config: &FileSearchConfig,
) -> Result<()> {
    let start_time = std::time::Instant::now();
    let mut total_files_with_hints = 0;
    let mut total_hints = 0;

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
    std::thread::sleep(std::time::Duration::from_secs(3));

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

        // Count lines in the file
        let line_count = file_content.lines().count() as u32;

        // Create the file URI
        let file_uri = format!("file://{}", absolute_path.display());

        // Request inlay hints for the entire file
        let inlay_hint_params = InlayHintParams {
            text_document: TextDocumentIdentifier {
                uri: file_uri.parse()?,
            },
            range: Range {
                start: lsp_types::Position {
                    line: 0,
                    character: 0,
                },
                end: lsp_types::Position {
                    line: line_count,
                    character: 0,
                },
            },
            work_done_progress_params: WorkDoneProgressParams {
                work_done_token: None,
            },
        };

        // Give LSP a moment after opening the file
        // std::thread::sleep(std::time::Duration::from_millis(100));

        // Send the inlay hint request with retry logic
        let before_request = std::time::Instant::now();
        let mut last_error = None;
        let mut hints_result = None;

        // Try up to 3 times with exponential backoff (0ms, 50ms, 250ms)
        for attempt in 0..3 {
            if attempt > 0 {
                let delay_ms = match attempt {
                    1 => 50,
                    2 => 250,
                    _ => 0,
                };
                tracing::debug!("Retry attempt {} after {}ms delay", attempt + 1, delay_ms);
                std::thread::sleep(std::time::Duration::from_millis(delay_ms));
            }

            match lsp_server
                .request::<lsp_types::request::InlayHintRequest>(inlay_hint_params.clone())
            {
                Ok(result) => {
                    hints_result = Some(result);
                    break;
                }
                Err(e) if e.to_string().contains("content modified") && attempt < 2 => {
                    tracing::debug!(
                        "Got 'content modified' error on attempt {}, retrying...",
                        attempt + 1
                    );
                    last_error = Some(e);
                    continue;
                }
                Err(e) => {
                    last_error = Some(e);
                    break;
                }
            }
        }

        match hints_result {
            Some(Some(hints)) => {
                let request_time = before_request.elapsed();
                println!(
                    "\nFound {} inlay hints in {:.2?}",
                    hints.len(),
                    request_time
                );

                total_files_with_hints += 1;
                total_hints += hints.len();

                // Display each hint
                if !hints.is_empty() {
                    println!("\nInlay Hints:");
                    println!("{}", "-".repeat(80));

                    // Split the file content into lines for display
                    let lines: Vec<&str> = file_content.lines().collect();

                    for hint in &hints {
                        let line_num = hint.position.line as usize;
                        let char_pos = hint.position.character as usize;

                        // Get the line content if available
                        let line_content = if line_num < lines.len() {
                            lines[line_num].trim()
                        } else {
                            ""
                        };

                        // Format the hint label
                        let label = match &hint.label {
                            lsp_types::InlayHintLabel::String(s) => s.clone(),
                            lsp_types::InlayHintLabel::LabelParts(parts) => {
                                parts.iter().map(|p| p.value.as_str()).collect::<String>()
                            }
                        };

                        // Determine hint kind
                        let kind = match hint.kind {
                            Some(lsp_types::InlayHintKind::TYPE) => "Type",
                            Some(lsp_types::InlayHintKind::PARAMETER) => "Parameter",
                            _ => "Other",
                        };

                        // Display the hint
                        println!("  Line {}:{} [{}]: {}", line_num + 1, char_pos, kind, label);

                        // Show a snippet of the line for context
                        if !line_content.is_empty() {
                            println!("    Context: {}", line_content);
                        }

                        // Add padding hint if available
                        if hint.padding_left == Some(true) || hint.padding_right == Some(true) {
                            let padding = match (hint.padding_left, hint.padding_right) {
                                (Some(true), Some(true)) => " (with padding left & right)",
                                (Some(true), _) => " (with padding left)",
                                (_, Some(true)) => " (with padding right)",
                                _ => "",
                            };
                            println!("    {}", padding);
                        }

                        println!();
                    }
                }
            }
            Some(None) => {
                println!("\nNo inlay hints available for this file");
            }
            None => {
                if let Some(e) = last_error {
                    tracing::warn!("Failed to get inlay hints after retries: {}", e);
                    println!("\nError getting inlay hints: {}", e);
                } else {
                    println!("\nNo inlay hints available for this file");
                }
            }
        }

        // Close the document
        if let Err(e) = lsp_server.close_file(&absolute_path) {
            tracing::warn!(
                "Failed to close document {}: {}",
                absolute_path.display(),
                e
            );
        }
    }

    // Stop the LSP server
    tracing::info!("Stopping LSP server...");
    if let Err(e) = lsp_server.stop() {
        tracing::error!("Error stopping LSP server: {}", e);
    }

    // Print summary
    let elapsed = start_time.elapsed();
    println!("\n{}", "=".repeat(80));
    println!("Summary:");
    println!(
        "  Files with hints: {} / {}",
        total_files_with_hints,
        matching_files.len()
    );
    println!("  Total inlay hints: {}", total_hints);
    println!("  Time elapsed: {:.2?}", elapsed);
    println!("{}", "=".repeat(80));

    Ok(())
}

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Parse and validate command-line arguments
    let args = Args::parse_and_validate()?;

    // Create file search configuration
    let config = args.create_file_search_config()?;

    // Process files based on language
    match args.language.as_str() {
        "rust" => process_files(RustLang, &args.project_path, &config)?,
        "python" => process_files(PythonLang, &args.project_path, &config)?,
        "typescript" => process_files(TypeScriptLang, &args.project_path, &config)?,
        "go" => process_files(GoLang, &args.project_path, &config)?,
        "swift" => process_files(SwiftLang, &args.project_path, &config)?,
        _ => unreachable!("Language should have been validated"),
    }

    Ok(())
}
