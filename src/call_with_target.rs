use lsp_types::Location;

/// A call and its definition
#[derive(Debug, Clone)]
pub struct CallWithTarget {
    /// The path to the file containing the call
    pub file_path: std::path::PathBuf,
    /// The tree-sitter node representing the call
    pub call_node: tree_sitter::Node<'static>,
    /// The LSP definition response for the call
    pub definition: lsp_types::GotoDefinitionResponse,
}

fn pretty_print_location(call: &CallWithTarget, location: &Location) -> String {
    let call_pos = call.call_node.start_position();
    format!(
        "Call {}:{}:{} targets {}:{}:{}",
        call.file_path.display(),
        call_pos.row + 1,
        call_pos.column + 1,
        location.uri.path(),
        location.range.start.line + 1,
        location.range.start.character + 1
    )
}

impl CallWithTarget {
    pub fn pretty_print(&self) -> Vec<String> {
        match &self.definition {
            lsp_types::GotoDefinitionResponse::Scalar(location) => {
                vec![pretty_print_location(self, location)]
            }
            lsp_types::GotoDefinitionResponse::Array(locations) => locations
                .iter()
                .map(|loc| pretty_print_location(self, loc))
                .collect(),
            lsp_types::GotoDefinitionResponse::Link(_links) => {
                panic!("Definition links are not supported for pretty printing")
            }
        }
    }
}
