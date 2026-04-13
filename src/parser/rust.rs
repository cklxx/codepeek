use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set Rust language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    let root = tree.root_node();
    let lines: Vec<&str> = source.lines().collect();

    let mut functions = Vec::new();
    collect_functions(root, &lines, path, &mut functions);

    Ok(functions)
}

fn collect_functions(
    node: Node,
    lines: &[&str],
    path: &PathBuf,
    out: &mut Vec<FunctionInfo>,
) {
    if node.kind() == "function_item" {
        if let Some(info) = extract_function(node, lines, path) {
            out.push(info);
        }
    }

    // Recurse into impl blocks, modules, etc.
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_item" | "impl_item" | "mod_item" | "source_file" => {
                collect_functions(child, lines, path, out);
            }
            _ => {
                if child.named_child_count() > 0 {
                    collect_functions(child, lines, path, out);
                }
            }
        }
    }
}

fn extract_function(node: Node, lines: &[&str], path: &PathBuf) -> Option<FunctionInfo> {
    let start_line = node.start_position().row; // 0-indexed
    let end_line = node.end_position().row;

    // Get function name
    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(
        lines.join("\n").as_bytes(),
    ).ok()?.to_string();

    // Build signature: everything up to the opening brace
    let signature = build_signature(node, lines);

    // Extract doc comment from preceding lines
    let doc_comment = extract_doc_comment(start_line, lines);

    // Extract body lines (skip first/last which are `{` and `}`)
    let body_start = start_line + 1;
    let body_end = if end_line > body_start { end_line } else { body_start };
    let raw_body: Vec<String> = lines
        .get(body_start..body_end)
        .unwrap_or(&[])
        .iter()
        .map(|l| l.to_string())
        .collect();

    Some(FunctionInfo::new(
        name,
        signature,
        path.clone(),
        (start_line + 1, end_line + 1), // 1-indexed for display
        raw_body,
        doc_comment,
    ))
}

fn build_signature(node: Node, lines: &[&str]) -> String {
    let start = node.start_position();
    let start_line_text = lines.get(start.row).unwrap_or(&"");

    // Find the opening brace and take everything before it
    let sig_end = start_line_text.find('{').unwrap_or(start_line_text.len());
    let first_line = start_line_text[..sig_end].trim().to_string();

    // If signature spans multiple lines (where block starts later), include them
    let mut sig = first_line;
    if !sig.contains('{') {
        // Multi-line signature — just use first line for now
    }
    sig.trim_end_matches('{').trim().to_string()
}

fn extract_doc_comment(fn_start_line: usize, lines: &[&str]) -> Option<String> {
    if fn_start_line == 0 {
        return None;
    }

    let mut doc_lines = Vec::new();
    let mut i = fn_start_line as isize - 1;

    while i >= 0 {
        let line = lines[i as usize].trim();
        if line.starts_with("///") {
            doc_lines.push(line.trim_start_matches("///").trim().to_string());
            i -= 1;
        } else if line.starts_with("//!") || line.starts_with("#[") || line.is_empty() {
            i -= 1;
        } else {
            break;
        }
    }

    if doc_lines.is_empty() {
        return None;
    }

    doc_lines.reverse();
    Some(doc_lines.join(" ").chars().take(80).collect())
}
