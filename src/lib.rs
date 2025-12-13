// Module declarations
pub mod call_node;
pub mod call_with_target;
pub mod cli;
pub mod file_search;
pub mod integration;
pub mod language;
pub mod languages;
pub mod location;
pub mod lsp;
pub mod parser;

// Re-export main types
pub use cli::Args;
pub use file_search::FileSearchConfig;
pub use integration::{find_all_call_targets, goto_definition_for_node};
pub use language::Language;
pub use languages::{GoLang, PythonLang, RustLang, SwiftLang, TypeScriptLang};
pub use lsp::{LspServer, LspServerConfig};
