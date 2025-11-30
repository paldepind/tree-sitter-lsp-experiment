//! Find all files in a given directory that match a language's file pattern.

use anyhow::Result;
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

use crate::language::Language;

/// Configuration for file searching behavior
#[derive(Debug, Clone)]
pub struct FileSearchConfig {
    /// Directories to skip during recursive search
    pub skip_dirs: Vec<String>,
    /// Maximum depth for recursive search (None = unlimited)
    pub max_depth: Option<usize>,
    /// Optional glob pattern to filter files (None = no filtering)
    pub include_glob: Option<glob::Pattern>,
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
            include_glob: None,
        }
    }
}

impl FileSearchConfig {
    /// Recursively finds all files in the given directory that match the language's file pattern
    pub fn find_language_files(
        &self,
        dir_path: &Path,
        language: impl Language,
    ) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();
        let file_regex = language.file_regex()?;

        self.find_files_recursive(
            dir_path,
            &file_regex,
            &self.include_glob,
            &mut matching_files,
            0,
        )?;

        Ok(matching_files)
    }

    fn is_dir_skipped(&self, dir: &Path) -> bool {
        if let Some(dir_name) = dir.file_name().and_then(|n| n.to_str()) {
            self.skip_dirs.contains(&dir_name.to_string())
        } else {
            false
        }
    }
    /// Helper function to recursively traverse directories and find matching files
    fn find_files_recursive(
        &self,
        dir: &Path,
        regex: &Regex,
        glob_matcher: &Option<glob::Pattern>,
        results: &mut Vec<PathBuf>,
        current_depth: usize,
    ) -> Result<()> {
        if !dir.is_dir() {
            return Ok(());
        }

        // Check depth limit
        if self
            .max_depth
            .is_some_and(|max_depth| current_depth >= max_depth)
        {
            return Ok(());
        }

        let entries = fs::read_dir(dir)
            .map_err(|e| anyhow::anyhow!("Failed to read directory {}: {}", dir.display(), e))?;

        for entry in entries {
            let entry =
                entry.map_err(|e| anyhow::anyhow!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() && !self.is_dir_skipped(&path) {
                // Recursively search subdirectories
                self.find_files_recursive(&path, regex, glob_matcher, results, current_depth + 1)?;
            } else if path.is_file()
                && let Some(file_name) = path.file_name().and_then(|n| n.to_str())
                && regex.is_match(file_name)
            {
                // Check glob pattern if one is specified
                if let Some(pattern) = glob_matcher
                    && let Some(path_str) = path.to_str()
                {
                    if pattern.matches(path_str) {
                        results.push(path);
                    }
                } else {
                    results.push(path);
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::RustLang;

    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_search() -> Result<()> {
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

        let config = FileSearchConfig::default();
        let rust_files = config.find_language_files(temp_path, RustLang)?;

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
