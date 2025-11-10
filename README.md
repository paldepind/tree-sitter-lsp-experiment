# tree-sitter-lsp-experiment

## Supported Languages

- Rust
- Python
- TypeScript
- Go

## LSP Server Installation

The project relies on LSP servers being installed and available in the PATH.

### Rust - rust-analyzer

```sh
# Using rustup
rustup component add rust-analyzer
```

### Python - python-lsp-server

```sh
pip install python-lsp-server
```

**Optional plugins:**
```sh
pip install python-lsp-server[all]  # Install with all optional plugins
```

### TypeScript - typescript-language-server

```sh
npm install -g typescript-language-server typescript
```

### Go - gopls

```sh
go install golang.org/x/tools/gopls@latest
```

Make sure `$GOPATH/bin` is in your PATH.

## Usage

```sh
# Run on a project directory with a specific language
tree-sitter-lsp-experiment <project_path> --language <language>

# Example: Analyze a Rust project
tree-sitter-lsp-experiment ./my-rust-project --language rust

# Example: Analyze a Python project
tree-sitter-lsp-experiment ./my-python-project --language python
```

### Run with Debug Logging

```sh
RUST_LOG=debug cargo run -- <project_path> --language <language>
```