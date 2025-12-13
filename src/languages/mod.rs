//! Programming language implementations.

mod go;
mod python;
mod rust;
mod swift;
mod typescript;

pub use go::GoLang;
pub use python::PythonLang;
pub use rust::RustLang;
pub use swift::SwiftLang;
pub use typescript::TypeScriptLang;
