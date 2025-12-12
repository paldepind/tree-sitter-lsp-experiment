# tree-sitter-lsp-experiment

Experiment to evaluate and benchmark LSP server implementations.

## Supported Languages

- Rust
- Python
- TypeScript
- Go
- Swift

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

### Swift - `sourcekit-lsp`

On macOS [sourcekit-lsp](https://github.com/swiftlang/sourcekit-lsp) comes
bundled with Xcode or the Swift toolchain.

## Usage

```sh
cargo run --bin call-hierachy -- --help
```

### Run with Debug Logging

```sh
RUST_LOG=debug cargo run -- <project_path> --language <language>
```