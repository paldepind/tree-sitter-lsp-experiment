use std::path::PathBuf;

use lsp_types::Range;

pub fn highlight_range(file_lines: &[&str], range: Range) {
    let start_line = range.start.line as usize;
    let start_character = range.start.character as usize;
    let end_line = range.end.line as usize;
    let end_character = range.end.character as usize;

    if start_line < file_lines.len() {
        let line = file_lines[start_line];
        let line_len = line.len();
        println!("    {}", line.trim());

        let leading_spaces = line.chars().take_while(|c| c.is_whitespace()).count();
        let underline_width = if end_line == start_line {
            (end_character - start_character).max(1)
        } else {
            line_len - start_character
        };
        let mut call_underline = String::new();
        call_underline.push_str(" ".repeat(start_character - leading_spaces).as_str());
        call_underline.push_str("^".repeat(underline_width).as_str());
        print!("    {}", call_underline);
    }
}
