//! Example script that analyzes a file and pretty prints all function calls found.
//!
//! This script:
//! 1. Takes a file path as input
//! 2. Detects the language based on file extension
//! 3. Parses the file using tree-sitter
//! 4. Finds all function/method calls
//! 5. Pretty prints each call showing both the call node and goto definition node
//!
//! Usage: cargo run --example show_calls -- <file_path>

use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use tree_sitter_lsp_experiment::{
    GoLang, Language, PythonLang, RustLang, SwiftLang, TypeScriptLang,
};

/// Process a file with a specific language
fn process_file<L: Language>(file_path: &Path, language: L) -> Result<()> {
    println!("Detected language: {}", language);
    println!("File: {}\n", file_path.display());

    // Read file contents
    let source_code = fs::read_to_string(file_path)?;

    // Parse the file
    let tree = tree_sitter_lsp_experiment::parser::parse_file(file_path, language)?;

    // Get all calls
    let calls: Vec<_> = tree_sitter_lsp_experiment::parser::get_calls(&tree).collect();

    println!("Found {} call(s):\n", calls.len());

    // Split source into lines for display
    let source_lines: Vec<&str> = source_code.lines().collect();

    // Pretty print each call
    for (idx, call) in calls.iter().enumerate() {
        let line_num = call.call_node.start_position().row;
        let call_start_col = call.call_node.start_position().column;
        let call_end_col = call.call_node.end_position().column;
        let goto_start_col = call.goto_definition_node.start_position().column;
        let goto_end_col = call.goto_definition_node.end_position().column;

        // Only show if both call and goto are on the same line
        if call.call_node.start_position().row == call.call_node.end_position().row
            && call.goto_definition_node.start_position().row
                == call.goto_definition_node.end_position().row
            && call.call_node.start_position().row == call.goto_definition_node.start_position().row
        {
            if let Some(source_line) = source_lines.get(line_num) {
                println!("{}: {}", line_num + 1, source_line);

                // Create underline for call node
                let mut call_underline = String::new();
                call_underline.push_str(" ".repeat(call_start_col).as_str());
                call_underline.push_str("^".repeat(call_end_col - call_start_col).as_str());

                // Create underline for goto definition node
                let mut goto_underline = String::new();
                goto_underline.push_str(" ".repeat(goto_start_col).as_str());
                goto_underline.push_str("^".repeat(goto_end_col - goto_start_col).as_str());

                // Print with proper indentation (matching line number width)
                let indent = " ".repeat(format!("{}", line_num + 1).len() + 2);
                println!("{}{} call", indent, call_underline);
                println!("{}{} goto definition", indent, goto_underline);
                println!();
            }
        } else {
            // Multi-line call - show basic info
            println!(
                "Call #{}: line {} (multi-line, spans {}:{} to {}:{})",
                idx + 1,
                line_num + 1,
                call.call_node.start_position().row + 1,
                call.call_node.start_position().column,
                call.call_node.end_position().row + 1,
                call.call_node.end_position().column
            );
            println!();
        }
    }

    Ok(())
}

fn main() -> Result<()> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <file_path>", args[0]);
        eprintln!("Supported extensions: .rs, .py, .ts, .tsx, .go, .swift");
        std::process::exit(1);
    }

    let file_path = PathBuf::from(&args[1]);

    // Verify the file exists
    if !file_path.exists() {
        anyhow::bail!("File does not exist: {}", file_path.display());
    }

    if !file_path.is_file() {
        anyhow::bail!("Path is not a file: {}", file_path.display());
    }

    // Detect language from file extension and process
    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .ok_or_else(|| anyhow::anyhow!("File has no extension"))?;

    match extension {
        "rs" => process_file(&file_path, RustLang),
        "py" => process_file(&file_path, PythonLang),
        "ts" | "tsx" => process_file(&file_path, TypeScriptLang),
        "go" => process_file(&file_path, GoLang),
        "swift" => process_file(&file_path, SwiftLang),
        _ => Err(anyhow::anyhow!(
            "Unsupported file extension: .{}",
            extension
        )),
    }
}
