use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

// Re-export LSP functionality
pub mod lsp;
pub use lsp::{LspServer, LspServerConfig, LspServerManager, start_lsp_server};

/// Supported programming languages for Tree Sitter parsing and LSP integration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    Go,
}

impl FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "rust" => Ok(Language::Rust),
            "python" => Ok(Language::Python),
            "typescript" => Ok(Language::TypeScript),
            "go" => Ok(Language::Go),
            _ => anyhow::bail!(
                "Unsupported language: {}. Supported languages: rust, python, typescript, go",
                s
            ),
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::TypeScript => "TypeScript",
            Language::Go => "Go",
        };
        write!(f, "{}", name)
    }
}

impl Language {
    /// Returns a regex pattern that matches files for this language
    pub fn file_pattern(&self) -> &'static str {
        match self {
            Language::Rust => r"\.rs$",
            Language::Python => r"\.py$",
            Language::TypeScript => r"\.(ts|tsx)$",
            Language::Go => r"\.go$",
        }
    }

    /// Creates a compiled regex for matching files of this language
    pub fn file_regex(&self) -> Result<Regex> {
        Regex::new(self.file_pattern())
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))
    }

    /// Returns the file extensions for this language as a human-readable string
    pub fn extensions(&self) -> &'static str {
        match self {
            Language::Rust => ".rs",
            Language::Python => ".py",
            Language::TypeScript => ".ts, .tsx",
            Language::Go => ".go",
        }
    }

    /// Returns all supported languages
    pub fn all() -> Vec<Language> {
        vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
        ]
    }

    /// Returns the lowercase name used for command line arguments
    pub fn cli_name(&self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::TypeScript => "typescript",
            Language::Go => "go",
        }
    }
}

/// Configuration for file searching behavior
#[derive(Debug, Clone)]
pub struct FileSearchConfig {
    /// Directories to skip during recursive search
    pub skip_dirs: Vec<String>,
    /// Maximum depth for recursive search (None = unlimited)
    pub max_depth: Option<usize>,
}

impl Default for FileSearchConfig {
    fn default() -> Self {
        Self {
            skip_dirs: vec![
                "node_modules".to_string(),
                "target".to_string(),
                ".git".to_string(),
                "dist".to_string(),
                "build".to_string(),
                "__pycache__".to_string(),
                ".next".to_string(),
                "vendor".to_string(),
                ".venv".to_string(),
                "venv".to_string(),
            ],
            max_depth: None,
        }
    }
}

/// File finder for locating source code files by language
pub struct FileFinder {
    config: FileSearchConfig,
}

impl FileFinder {
    /// Create a new FileFinder with default configuration
    pub fn new() -> Self {
        Self {
            config: FileSearchConfig::default(),
        }
    }

    /// Create a new FileFinder with custom configuration
    pub fn with_config(config: FileSearchConfig) -> Self {
        Self { config }
    }

    /// Recursively finds all files in the given directory that match the language's file pattern
    pub fn find_language_files(
        &self,
        dir_path: &Path,
        language: &Language,
    ) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();
        let file_regex = language.file_regex()?;

        self.find_files_recursive(dir_path, &file_regex, &mut matching_files, 0)?;

        Ok(matching_files)
    }

    /// Helper function to recursively traverse directories and find matching files
    fn find_files_recursive(
        &self,
        dir: &Path,
        regex: &Regex,
        results: &mut Vec<PathBuf>,
        current_depth: usize,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Check depth limit
        if let Some(max_depth) = self.config.max_depth {
            if current_depth >= max_depth {
                return Ok(());
            }
        }

        let entries = fs::read_dir(dir)
            .map_err(|e| anyhow::anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| anyhow::anyhow!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Check if directory should be skipped
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if self.config.skip_dirs.contains(&dir_name.to_string()) {
                        continue;
                    }
                }
                // Recursively search subdirectories
                self.find_files_recursive(&path, regex, results, current_depth + 1)?;
            } else if path.is_file() {
                // Check if the file matches our regex
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if regex.is_match(file_name) {
                        results.push(path);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for FileFinder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_language_from_str() {
        assert_eq!(Language::from_str("rust").unwrap(), Language::Rust);
        assert_eq!(Language::from_str("RUST").unwrap(), Language::Rust);
        assert_eq!(Language::from_str("python").unwrap(), Language::Python);
        assert_eq!(
            Language::from_str("typescript").unwrap(),
            Language::TypeScript
        );
        assert_eq!(Language::from_str("go").unwrap(), Language::Go);

        assert!(Language::from_str("java").is_err());
    }

    #[test]
    fn test_language_patterns() {
        assert_eq!(Language::Rust.file_pattern(), r"\.rs$");
        assert_eq!(Language::Python.file_pattern(), r"\.py$");
        assert_eq!(Language::TypeScript.file_pattern(), r"\.(ts|tsx)$");
        assert_eq!(Language::Go.file_pattern(), r"\.go$");
    }

    #[test]
    fn test_file_regex() {
        let rust_regex = Language::Rust.file_regex().unwrap();
        assert!(rust_regex.is_match("main.rs"));
        assert!(rust_regex.is_match("lib.rs"));
        assert!(!rust_regex.is_match("main.py"));
        assert!(!rust_regex.is_match("main.rs.bak"));

        let ts_regex = Language::TypeScript.file_regex().unwrap();
        assert!(ts_regex.is_match("app.ts"));
        assert!(ts_regex.is_match("component.tsx"));
        assert!(!ts_regex.is_match("app.js"));
    }

    #[test]
    fn test_file_finder() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let temp_path = temp_dir.path();

        // Create test directory structure
        fs::create_dir_all(temp_path.join("src"))?;
        fs::create_dir_all(temp_path.join("tests"))?;
        fs::create_dir_all(temp_path.join("target/debug"))?; // Should be skipped

        // Create test files
        fs::write(temp_path.join("src/main.rs"), "fn main() {}")?;
        fs::write(temp_path.join("src/lib.rs"), "pub fn hello() {}")?;
        fs::write(
            temp_path.join("tests/integration.rs"),
            "#[test] fn test() {}",
        )?;
        fs::write(temp_path.join("target/debug/build.rs"), "// build script")?; // Should be skipped
        fs::write(temp_path.join("README.md"), "# Project")?; // Should not match

        let finder = FileFinder::new();
        let rust_files = finder.find_language_files(temp_path, &Language::Rust)?;

        assert_eq!(rust_files.len(), 3); // main.rs, lib.rs, integration.rs (target/debug/build.rs should be skipped)

        // Verify target directory was skipped
        let filenames: Vec<String> = rust_files
            .iter()
            .filter_map(|p| p.file_name())
            .filter_map(|n| n.to_str())
            .map(|s| s.to_string())
            .collect();
        assert!(!filenames.contains(&"build.rs".to_string()));

        Ok(())
    }
}
