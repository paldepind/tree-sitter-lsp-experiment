use anyhow::Result;
use regex::Regex;
use std::str::FromStr;

/// Configuration for a language
struct LanguageConfig {
    display_name: &'static str,
    cli_name: &'static str,
    file_pattern: &'static str,
    extensions: &'static str,
}

/// Supported programming languages for Tree Sitter parsing and LSP integration
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Language {
    Rust,
    Python,
    TypeScript,
    Go,
    Swift,
}

impl Language {
    /// Returns the configuration for this language
    const fn config(&self) -> LanguageConfig {
        match self {
            Language::Rust => LanguageConfig {
                display_name: "Rust",
                cli_name: "rust",
                file_pattern: r"\.rs$",
                extensions: ".rs",
            },
            Language::Python => LanguageConfig {
                display_name: "Python",
                cli_name: "python",
                file_pattern: r"\.py$",
                extensions: ".py",
            },
            Language::TypeScript => LanguageConfig {
                display_name: "TypeScript",
                cli_name: "typescript",
                file_pattern: r"\.(ts|tsx)$",
                extensions: ".ts, .tsx",
            },
            Language::Go => LanguageConfig {
                display_name: "Go",
                cli_name: "go",
                file_pattern: r"\.go$",
                extensions: ".go",
            },
            Language::Swift => LanguageConfig {
                display_name: "Swift",
                cli_name: "swift",
                file_pattern: r"\.swift$",
                extensions: ".swift",
            },
        }
    }

    /// Returns all supported languages
    pub fn all() -> Vec<Language> {
        vec![
            Language::Rust,
            Language::Python,
            Language::TypeScript,
            Language::Go,
            Language::Swift,
        ]
    }

    /// Returns a regex pattern that matches files for this language
    pub fn file_pattern(&self) -> &'static str {
        self.config().file_pattern
    }

    /// Creates a compiled regex for matching files of this language
    pub fn file_regex(&self) -> Result<Regex> {
        Regex::new(self.file_pattern())
            .map_err(|e| anyhow::anyhow!("Failed to compile regex: {}", e))
    }

    /// Returns the file extensions for this language as a human-readable string
    pub fn extensions(&self) -> &'static str {
        self.config().extensions
    }

    /// Returns the lowercase name used for command line arguments
    pub fn cli_name(&self) -> &'static str {
        self.config().cli_name
    }
}

impl FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lower = s.to_lowercase();
        Language::all()
            .into_iter()
            .find(|lang| lang.cli_name() == s_lower)
            .ok_or_else(|| {
                let supported = Language::all()
                    .iter()
                    .map(|l| l.cli_name())
                    .collect::<Vec<_>>()
                    .join(", ");
                anyhow::anyhow!(
                    "Unsupported language: {}. Supported languages: {}",
                    s,
                    supported
                )
            })
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config().display_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
