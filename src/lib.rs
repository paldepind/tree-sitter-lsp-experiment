// Module declarations
pub mod file_search;
pub mod integration;
pub mod language;
pub mod lsp;
pub mod parser;

// Re-export main types
pub use file_search::FileSearchConfig;
pub use language::Language;
pub use lsp::{LspServer, LspServerConfig};
pub use parser::parse_file;
