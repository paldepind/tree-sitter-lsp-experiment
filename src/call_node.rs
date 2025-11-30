use tree_sitter::Node;

pub struct CallNode<'tree> {
    // The node representing the function/method call
    pub call_node: Node<'tree>,
    // The node for which goto definiton should be performed
    pub goto_definition_node: Node<'tree>,
}

impl<'tree> CallNode<'tree> {
    /// Pretty prints the call node with visual indicators for the call and goto definition ranges
    ///
    /// This method displays the source line with underline markers showing where the call
    /// and goto definition nodes are located. If the nodes span multiple lines, it returns
    /// a simple multi-line indicator instead.
    ///
    /// # Arguments
    /// * `source_lines` - All lines of source code as a slice of string slices
    ///
    /// # Returns
    /// A vector of strings representing the pretty-printed output, or None if the call
    /// spans multiple lines
    pub fn pretty_print(&self, source_lines: &[&str]) -> Option<Vec<String>> {
        let line_num = self.call_node.start_position().row;
        let call_start_col = self.call_node.start_position().column;
        let call_end_col = self.call_node.end_position().column;
        let goto_start_col = self.goto_definition_node.start_position().column;
        let goto_end_col = self.goto_definition_node.end_position().column;

        // Only show if both call and goto are on the same line
        if self.call_node.start_position().row == self.call_node.end_position().row
            && self.goto_definition_node.start_position().row
                == self.goto_definition_node.end_position().row
            && self.call_node.start_position().row == self.goto_definition_node.start_position().row
            && let Some(source_line) = source_lines.get(line_num)
        {
            let mut output = Vec::new();

            // Source line with line number
            output.push(format!("{}: {}", line_num + 1, source_line));

            // Create underline for call node
            let mut call_underline = String::new();
            call_underline.push_str(" ".repeat(call_start_col).as_str());
            call_underline.push_str("^".repeat(call_end_col - call_start_col).as_str());

            // Create underline for goto definition node
            let mut goto_underline = String::new();
            goto_underline.push_str(" ".repeat(goto_start_col).as_str());
            goto_underline.push_str("~".repeat(goto_end_col - goto_start_col).as_str());

            // Print with proper indentation (matching line number width)
            let indent = " ".repeat(format!("{}", line_num + 1).len() + 2);
            output.push(format!("{}{} call", indent, call_underline));
            output.push(format!("{}{} goto definition", indent, goto_underline));

            Some(output)
        } else {
            None
        }
    }
}
