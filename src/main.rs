use anyhow::Result;
use std::env;
use std::path::PathBuf;
use std::str::FromStr;
use tree_sitter_lsp_experiment::{FileFinder, Language, start_lsp_server};

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
    let mut language: Option<Language> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--language" => {
                if i + 1 >= args.len() {
                    eprintln!("Error: --language requires a value");
                    std::process::exit(1);
                }
                language = Some(Language::from_str(&args[i + 1])?);
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

    tracing::info!(
        "Starting LSP experiment with project: {} (Language: {}, Extensions: {})",
        project_path.display(),
        language,
        language.extensions()
    );

    // Create the regex for matching files of the specified language
    let _file_regex = language.file_regex()?;
    tracing::debug!("File pattern for {}: {}", language, language.file_pattern());

    // Find all files of the specified language in the project
    tracing::info!("Scanning for {} files in project...", language);
    let finder = FileFinder::new();
    let matching_files = finder.find_language_files(&project_path, language)?;

    tracing::info!("Found {} {} files:", matching_files.len(), language);
    for file in &matching_files {
        tracing::info!("  {}", file.display());
    }

    // Start LSP server for the language
    tracing::info!("Starting LSP server for {}...", language);
    match start_lsp_server(language, &project_path) {
        Ok(mut lsp_server) => {
            tracing::info!("LSP server started successfully!");
            tracing::info!(
                "LSP server is running in: {}",
                lsp_server.working_dir.display()
            );

            // Keep the server running for a bit to demonstrate
            std::thread::sleep(std::time::Duration::from_secs(2));

            // Stop the server
            tracing::info!("Stopping LSP server...");
            if let Err(e) = lsp_server.stop() {
                tracing::error!("Error stopping LSP server: {}", e);
            }
        }
        Err(e) => {
            tracing::error!("Failed to start LSP server: {}", e);
            return Err(e);
        }
    }

    // TODO: Use matching_files to process each file with Tree Sitter
    // TODO: Implement LSP client communication

    Ok(())
}
