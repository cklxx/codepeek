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
    let src = source.as_bytes();

    let mut functions = Vec::new();
    collect_functions(root, &lines, src, path, &mut functions);
    Ok(functions)
}

fn collect_functions(
    node: Node,
    lines: &[&str],
    src: &[u8],
    path: &PathBuf,
    out: &mut Vec<FunctionInfo>,
) {
    if node.kind() == "function_item" {
        if let Some(info) = extract_function(node, lines, src, path) {
            out.push(info);
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, lines, src, path, out);
    }
}

fn extract_function(
    node: Node,
    lines: &[&str],
    src: &[u8],
    _path: &PathBuf,
) -> Option<FunctionInfo> {
    let start = node.start_position().row;
    let end = node.end_position().row;

    let name = node.child_by_field_name("name")?.utf8_text(src).ok()?.to_string();
    let signature = build_signature(node, lines);
    let doc = extract_doc_comment(start, lines);

    // First non-empty body line for fallback summary
    let first_body = lines
        .get(start + 1..end)
        .unwrap_or(&[])
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string());

    Some(FunctionInfo::new(
        name,
        signature,
        (start + 1, end + 1),
        doc,
        first_body,
    ))
}

fn build_signature(node: Node, lines: &[&str]) -> String {
    let start = node.start_position().row;
    let end = node.end_position().row;

    // Collect lines until we hit the opening `{`
    let mut sig_lines = Vec::new();
    for row in start..=end {
        let line = lines.get(row).unwrap_or(&"");
        if let Some(pos) = line.find('{') {
            sig_lines.push(line[..pos].trim_end().to_string());
            break;
        } else {
            sig_lines.push(line.trim_end().to_string());
        }
    }
    sig_lines.join(" ").trim().to_string()
}

fn extract_doc_comment(fn_start: usize, lines: &[&str]) -> Option<String> {
    if fn_start == 0 {
        return None;
    }
    let mut doc = Vec::new();
    let mut i = fn_start as isize - 1;
    while i >= 0 {
        let l = lines[i as usize].trim();
        if l.starts_with("///") {
            doc.push(l.trim_start_matches("///").trim().to_string());
        } else if l.starts_with("#[") || l.is_empty() {
            // skip attrs and blanks
        } else {
            break;
        }
        i -= 1;
    }
    if doc.is_empty() {
        return None;
    }
    doc.reverse();
    Some(doc.join(" ").chars().take(100).collect())
}
