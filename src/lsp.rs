use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::Language;

/// Configuration for LSP server startup
#[derive(Debug, Clone)]
pub struct LspServerConfig {
    /// Working directory for the LSP server
    pub working_dir: PathBuf,
    /// Additional arguments to pass to the LSP server
    pub args: Vec<String>,
    /// Environment variables to set for the LSP server
    pub env_vars: Vec<(String, String)>,
}

impl LspServerConfig {
    pub fn new(working_dir: PathBuf) -> Self {
        Self {
            working_dir,
            args: Vec::new(),
            env_vars: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_env_vars(mut self, env_vars: Vec<(String, String)>) -> Self {
        self.env_vars = env_vars;
        self
    }
}

/// A running LSP server process
pub struct LspServer {
    pub process: Child,
    pub language: Language,
    pub working_dir: PathBuf,
}

impl LspServer {
    /// Stops the LSP server process
    pub fn stop(&mut self) -> Result<()> {
        tracing::info!(
            "Stopping LSP server for {} (PID: {:?})",
            self.language,
            self.process.id()
        );

        match self.process.kill() {
            Ok(_) => {
                if let Ok(exit_status) = self.process.wait() {
                    tracing::info!("LSP server terminated with status: {}", exit_status);
                }
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to kill LSP server process: {}", e);
                Err(anyhow::anyhow!("Failed to stop LSP server: {}", e))
            }
        }
    }
}

impl Drop for LspServer {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            tracing::error!("Error stopping LSP server in drop: {}", e);
        }
    }
}

/// LSP server manager for starting language-specific servers
pub struct LspServerManager;

impl LspServerManager {
    /// Starts an LSP server for the specified language in the given directory
    pub fn start_server(language: &Language, config: LspServerConfig) -> Result<LspServer> {
        tracing::info!(
            "Starting LSP server for {} in {}",
            language,
            config.working_dir.display()
        );

        let (command, args) = Self::get_server_command(language)?;

        let mut cmd = Command::new(&command);
        cmd.current_dir(&config.working_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .args(&args)
            .args(&config.args);

        // Set environment variables
        for (key, value) in &config.env_vars {
            cmd.env(key, value);
        }

        tracing::debug!("Executing command: {} {}", command, args.join(" "));

        let process = cmd.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start LSP server '{}': {}. Make sure the LSP server is installed and available in PATH.", command, e))?;

        tracing::info!(
            "LSP server for {} started successfully (PID: {:?})",
            language,
            process.id()
        );

        Ok(LspServer {
            process,
            language: language.clone(),
            working_dir: config.working_dir,
        })
    }

    /// Returns the command and arguments needed to start an LSP server for the given language
    fn get_server_command(language: &Language) -> Result<(String, Vec<String>)> {
        match language {
            Language::Rust => Ok(("rust-analyzer".to_string(), vec![])),
            Language::Python => Ok(("pylsp".to_string(), vec![])), // Python LSP Server (pylsp)
            Language::TypeScript => Ok(("typescript-language-server".to_string(), vec![
                "--stdio".to_string(),
            ])),
            Language::Go => Ok(("gopls".to_string(), vec![])),
            Language::Swift => Ok(("sourcekit-lsp".to_string(), vec![])),
        }
    }

    /// Checks if the required LSP server is available for the given language
    pub fn is_server_available(language: &Language) -> bool {
        let (command, _) = match Self::get_server_command(language) {
            Ok(cmd) => cmd,
            Err(_) => return false,
        };

        // Try to execute the command with --version or --help to check availability
        match Command::new(&command).arg("--version").output() {
            Ok(_) => true,
            Err(_) => {
                // Try --help as fallback
                Command::new(&command).arg("--help").output().is_ok()
            }
        }
    }

    /// Returns installation instructions for the LSP server for the given language
    pub fn get_installation_instructions(language: &Language) -> &'static str {
        match language {
            Language::Rust => {
                "Install rust-analyzer: https://rust-analyzer.github.io/manual.html#installation"
            }
            Language::Python => "Install Python LSP Server: pip install python-lsp-server",
            Language::TypeScript => {
                "Install TypeScript Language Server: npm install -g typescript-language-server typescript"
            }
            Language::Go => "Install gopls: go install golang.org/x/tools/gopls@latest",
            Language::Swift => {
                "Install sourcekit-lsp: Install Xcode or Swift toolchain from https://swift.org/download/"
            }
        }
    }
}

/// Convenience function to start an LSP server for a language in a directory
pub fn start_lsp_server(language: &Language, path: &Path) -> Result<LspServer> {
    // Check if the LSP server is available
    if !LspServerManager::is_server_available(language) {
        let instructions = LspServerManager::get_installation_instructions(language);
        return Err(anyhow::anyhow!(
            "LSP server for {} is not available. {}",
            language,
            instructions
        ));
    }

    let config = LspServerConfig::new(path.to_path_buf());
    LspServerManager::start_server(language, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_server_installation_instructions() {
        // Test that each language has installation instructions
        for language in crate::Language::all() {
            let instructions = LspServerManager::get_installation_instructions(&language);
            assert!(
                !instructions.is_empty(),
                "Missing installation instructions for {}",
                language
            );
            assert!(
                instructions.contains(&language.to_string().to_lowercase())
                    || instructions.contains(&language.cli_name()),
                "Installation instructions should mention the language: {}",
                language
            );
        }
    }
}
