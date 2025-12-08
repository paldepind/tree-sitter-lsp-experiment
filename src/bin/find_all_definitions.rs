//! Example demonstrating how to find all function calls and their definitions in a project.
//!
//! Usage: cargo run --example find_all_definitions -- <project_path> --language <language>

use anyhow::Result;
use std::env;
use std::path::PathBuf;
use tree_sitter_lsp_experiment::{
    GoLang, PythonLang, RustLang, SwiftLang, TypeScriptLang, find_all_call_targets,
};

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
    let mut config = tree_sitter_lsp_experiment::FileSearchConfig::default();
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
        "Finding all function calls and their definitions in {}",
        project_path.display()
    );

    // Initialize performance timer
    let start_time = std::time::Instant::now();

    // Find all calls and their definitions
    let results = match language {
        "rust" => find_all_call_targets(RustLang, &project_path, &config)?,
        "python" => find_all_call_targets(PythonLang, &project_path, &config)?,
        "typescript" => find_all_call_targets(TypeScriptLang, &project_path, &config)?,
        "go" => find_all_call_targets(GoLang, &project_path, &config)?,
        "swift" => find_all_call_targets(SwiftLang, &project_path, &config)?,
        lang => anyhow::bail!("Unsupported language: {}.", lang),
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
