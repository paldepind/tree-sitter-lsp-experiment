//! Example demonstrating how to find all function calls and their definitions in a project.
//!
//! Usage: cargo run --bin goto-definition -- <project_path> --language <language>

use anyhow::Result;
use tree_sitter_lsp_experiment::{
    Args, GoLang, PythonLang, RustLang, SwiftLang, TypeScriptLang, find_all_call_targets,
};

fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Parse and validate command-line arguments
    let args = Args::parse_and_validate()?;
    let config = args.create_file_search_config()?;

    println!(
        "Finding all function calls and their definitions in {}",
        args.project_path.display()
    );

    // Initialize performance timer
    let start_time = std::time::Instant::now();

    // Find all calls and their definitions
    let results = match args.language.as_str() {
        "rust" => find_all_call_targets(RustLang, &args.project_path, &config)?,
        "python" => find_all_call_targets(PythonLang, &args.project_path, &config)?,
        "typescript" => find_all_call_targets(TypeScriptLang, &args.project_path, &config)?,
        "go" => find_all_call_targets(GoLang, &args.project_path, &config)?,
        "swift" => find_all_call_targets(SwiftLang, &args.project_path, &config)?,
        _ => unreachable!(),
    };

    for call in &results.calls_with_targets {
        for line in call.pretty_print() {
            println!("{}", line);
        }
    }

    let elapsed = start_time.elapsed();
    let ops_per_sec = (results.total_calls as f64) / elapsed.as_secs_f64();
    println!("\n{}", "=".repeat(80));
    println!(
        "Summary: {} calls with definitions found out of {} total calls in {:.2?}, {:.2} ops/sec",
        results.calls_with_targets.len(),
        results.total_calls,
        elapsed,
        ops_per_sec
    );

    Ok(())
}
