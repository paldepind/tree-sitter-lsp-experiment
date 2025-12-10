//! Shared command-line argument parsing for all binaries.

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use crate::FileSearchConfig;

/// Common command-line arguments for all LSP experiment binaries
#[derive(Parser, Debug)]
#[command(version, about)]
pub struct Args {
    /// Path to the project directory to analyze
    pub project_path: PathBuf,

    /// Programming language to analyze
    #[arg(short, long, value_name = "LANGUAGE")]
    pub language: String,

    /// Glob pattern to include specific files (e.g., '**/src/**')
    #[arg(long, value_name = "PATTERN")]
    pub include: Option<String>,

    /// Glob pattern to exclude specific files (e.g., '**/*test*')
    #[arg(long, value_name = "PATTERN")]
    pub exclude: Option<String>,
}

impl Args {
    /// Parse command-line arguments and validate inputs
    pub fn parse_and_validate() -> Result<Self> {
        let args = Self::parse();

        // Verify the project path exists
        if !args.project_path.exists() {
            anyhow::bail!(
                "Project path does not exist: {}",
                args.project_path.display()
            );
        }

        if !args.project_path.is_dir() {
            anyhow::bail!(
                "Project path is not a directory: {}",
                args.project_path.display()
            );
        }

        // Validate language
        match args.language.as_str() {
            "rust" | "python" | "typescript" | "go" | "swift" => {}
            _ => anyhow::bail!(
                "Unsupported language: '{}'. Supported languages: rust, python, typescript, go, swift",
                args.language
            ),
        }

        Ok(args)
    }

    /// Create a FileSearchConfig from the include/exclude patterns
    pub fn create_file_search_config(&self) -> Result<FileSearchConfig> {
        let mut config = FileSearchConfig::default();

        if let Some(pattern) = &self.include {
            let glob_pattern = glob::Pattern::new(pattern).map_err(|e| {
                anyhow::anyhow!("Invalid include glob pattern '{}': {}", pattern, e)
            })?;
            config.include_glob = Some(glob_pattern);
            println!("Using include pattern: {}", pattern);
        }

        if let Some(pattern) = &self.exclude {
            let glob_pattern = glob::Pattern::new(pattern).map_err(|e| {
                anyhow::anyhow!("Invalid exclude glob pattern '{}': {}", pattern, e)
            })?;
            config.exclude_glob = Some(glob_pattern);
            println!("Using exclude pattern: {}", pattern);
        }

        Ok(config)
    }
}
