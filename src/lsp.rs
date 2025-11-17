//! Provides `LspServer` as a type that represents a running LSP server as well
//! as convenience functions for communicating with it.

use anyhow::Result;
use lsp_types::notification::Notification;
use lsp_types::request::Request;
use serde_json::{from_value, to_value};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::Language;

/// Configuration for LSP server startup
#[derive(Debug, Clone, Default)]
pub struct LspServerConfig {
    /// Additional arguments to pass to the LSP server
    pub args: Vec<String>,
    /// Environment variables to set for the LSP server
    pub env_vars: Vec<(String, String)>,
}

/// A running LSP server process
pub struct LspServer {
    pub process: Child,
    pub language: Language,
    pub working_dir: PathBuf,
    pub stdin: ChildStdin,
    pub stdout: BufReader<ChildStdout>,
    next_id: u64,
}

fn request_string<T: serde::Serialize>(request: &T) -> Result<String> {
    let request_str = serde_json::to_string(&request)?;
    Ok(format!(
        "Content-Length: {}\r\n\r\n{}",
        request_str.len(),
        request_str
    ))
}

/// Returns installation instructions for the LSP server for the given language
fn get_installation_instructions(language: Language) -> &'static str {
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

/// Checks if the required LSP server is available for the given language
fn is_server_available(language: Language) -> bool {
    let (command, _) = language.lsp_server_command();

    // Try to execute the command with --version or --help to check availability
    match Command::new(command).arg("--version").output() {
        Ok(_) => true,
        Err(_) => {
            // Try with --help as a fallback
            Command::new(command).arg("--help").output().is_ok()
        }
    }
}

impl LspServer {
    /// Sends a request to the LSP server with an auto-incrementing ID
    pub fn send_request<R: Request>(&mut self, params: R::Params) -> Result<u64> {
        let id = self.next_id;
        self.next_id += 1;
        self.send_request_with_id::<R>(id, params)?;
        Ok(id)
    }

    /// Sends a request to the LSP server with a specific ID
    pub fn send_request_with_id<R: Request>(&mut self, id: u64, params: R::Params) -> Result<()> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": to_value(&params)?
        });

        let message = request_string(&request)?;
        let request_str = serde_json::to_string(&request)?;

        tracing::debug!("Sending request: {}", request_str);
        self.stdin.write_all(message.as_bytes())?;
        self.stdin.flush()?;

        Ok(())
    }

    /// Sends a notification to the LSP server
    pub fn send_notification<N: Notification>(&mut self, params: N::Params) -> Result<()> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": N::METHOD,
            "params": to_value(&params)?
        });

        let message = request_string(&notification)?;
        let notification_str = serde_json::to_string(&notification)?;

        tracing::debug!("Sending notification: {}", notification_str);
        self.stdin.write_all(message.as_bytes())?;
        self.stdin.flush()?;

        Ok(())
    }

    /// Reads a response from the LSP server
    pub fn read_response(&mut self) -> Result<serde_json::Value> {
        // Read headers
        let mut content_length = 0;
        loop {
            let mut header = String::new();
            self.stdout.read_line(&mut header)?;

            if header == "\r\n" {
                break;
            }

            if let Some(length_str) = header.strip_prefix("Content-Length: ") {
                content_length = length_str.trim().parse()?;
            }
        }

        // Read content
        let mut buffer = vec![0; content_length];
        std::io::Read::read_exact(&mut self.stdout, &mut buffer)?;

        let response_str = String::from_utf8(buffer)?;
        tracing::debug!("Received message: {}", response_str);

        let response: serde_json::Value = serde_json::from_str(&response_str)?;
        Ok(response)
    }

    /// Reads responses until finding one with the expected ID
    pub fn read_response_with_id(&mut self, expected_id: u64) -> Result<serde_json::Value> {
        // Keep reading messages until we find the response with the matching ID
        loop {
            let message = self.read_response()?;

            // Check if this is a notification (no id field) or response
            if let Some(id) = message.get("id") {
                if id.as_u64() == Some(expected_id) {
                    return Ok(message);
                } else {
                    tracing::debug!("Received response with different ID: {:?}", id);
                }
                // This is a notification or other message without an ID
            } else if let Some(method) = message.get("method") {
                tracing::debug!("Received notification: {}", method);
            }
        }
    }

    /// Sends a request and waits for the response
    pub fn request<R: Request>(&mut self, params: R::Params) -> Result<R::Result> {
        let id = self.send_request::<R>(params)?;
        let response = self.read_response_with_id(id)?;

        // Check if the response contains an error
        if let Some(error) = response.get("error") {
            let error_message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            let error_code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            return Err(anyhow::anyhow!(
                "LSP error (code {}): {}",
                error_code,
                error_message
            ));
        }

        // Extract the result field from the JSON-RPC response
        let result = response
            .get("result")
            .ok_or_else(|| anyhow::anyhow!("Missing result field in response"))?;

        // Deserialize into the request's result type
        let typed_result = from_value::<R::Result>(result.clone())?;
        Ok(typed_result)
    }

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

    /// Starts an LSP server for the specified language in the given directory
    pub fn start(
        language: Language,
        working_dir: PathBuf,
        config: LspServerConfig,
    ) -> Result<LspServer> {
        // Check if the LSP server is available
        if !is_server_available(language) {
            let instructions = get_installation_instructions(language);
            return Err(anyhow::anyhow!(
                "LSP server for {} is not available. {}",
                language,
                instructions
            ));
        }

        tracing::info!(
            "Starting LSP server for {} in {}",
            language,
            working_dir.display()
        );

        let (command, args) = language.lsp_server_command();

        let mut cmd = Command::new(command);
        cmd.current_dir(&working_dir)
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

        let mut process = cmd.spawn()
            .map_err(|e| anyhow::anyhow!("Failed to start LSP server '{}': {}. Make sure the LSP server is installed and available in PATH.", command, e))?;

        tracing::info!(
            "LSP server for {} started successfully (PID: {:?})",
            language,
            process.id()
        );

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdin"))?;
        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("Failed to get stdout"))?;
        let stdout = BufReader::new(stdout);

        // Spawn a thread to consume stderr to prevent the LSP server from blocking
        // when the stderr pipe fills up
        if let Some(stderr) = process.stderr.take() {
            let language_name = language.to_string();
            std::thread::spawn(move || {
                use std::io::BufRead;
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    tracing::debug!("[{} stderr] {}", language_name, line);
                }
            });
        }

        Ok(LspServer {
            process,
            language,
            working_dir,
            stdin,
            stdout,
            next_id: 1,
        })
    }
}

impl Drop for LspServer {
    fn drop(&mut self) {
        if let Err(e) = self.stop() {
            tracing::error!("Error stopping LSP server in drop: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_server_installation_instructions() {
        // Test that each language has installation instructions
        for language in crate::Language::all() {
            let instructions = get_installation_instructions(language);
            assert!(
                !instructions.is_empty(),
                "Missing installation instructions for {}",
                language
            );
            assert!(
                instructions.contains(&language.to_string().to_lowercase())
                    || instructions.contains(language.cli_name()),
                "Installation instructions should mention the language: {}",
                language
            );
        }
    }
}
