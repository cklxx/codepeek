use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let _ = path;
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
    collect_functions(root, &lines, src, None, &mut functions);
    Ok(functions)
}

/// Recurse the AST. `impl_owner` is the type name of the enclosing `impl` block, if any.
fn collect_functions<'a>(
    node: Node<'a>,
    lines: &[&str],
    src: &[u8],
    impl_owner: Option<&str>,
    out: &mut Vec<FunctionInfo>,
) {
    match node.kind() {
        "impl_item" => {
            // Extract the implementing type (last type child in `impl [Trait for] Type`)
            let type_name = node
                .child_by_field_name("type")
                .and_then(|n| n.utf8_text(src).ok())
                .map(|s| simplify_type(s));

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_functions(child, lines, src, type_name.as_deref(), out);
            }
            return;
        }
        "function_item" => {
            if let Some(mut info) = extract_function(node, lines, src) {
                info.owner = impl_owner.map(|s| s.to_string());
                out.push(info);
            }
            // Don't recurse into function bodies to avoid nested functions polluting the list
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, lines, src, impl_owner, out);
    }
}

fn extract_function(node: Node, lines: &[&str], src: &[u8]) -> Option<FunctionInfo> {
    let start = node.start_position().row;
    let end   = node.end_position().row;

    let name = node.child_by_field_name("name")?.utf8_text(src).ok()?.to_string();
    let signature = build_signature(node, lines);
    let doc = extract_doc_comment(start, lines);

    // Body lines for smart summary (skip first line = fn header)
    let body: Vec<&str> = lines
        .get(start + 1..end)
        .unwrap_or(&[])
        .iter()
        .copied()
        .collect();

    Some(FunctionInfo::new(name, signature, (start + 1, end + 1), doc, &body))
}

fn build_signature(node: Node, lines: &[&str]) -> String {
    let start = node.start_position().row;
    let end   = node.end_position().row;

    let mut sig_lines = Vec::new();
    for row in start..=end {
        let line = lines.get(row).unwrap_or(&"");
        // Stop at the opening brace of the body
        if let Some(pos) = line.find('{') {
            // But don't stop at `{` inside `where` clause type bounds
            let before = &line[..pos];
            sig_lines.push(before.trim_end().to_string());
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
            // skip attributes and blank lines between doc and fn
        } else {
            break;
        }
        i -= 1;
    }
    if doc.is_empty() {
        return None;
    }
    doc.reverse();
    Some(doc.join(" ").chars().take(120).collect())
}

/// Strip generic parameters from type names for display: `AppState<T>` → `AppState`
fn simplify_type(s: &str) -> String {
    s.find('<').map(|i| s[..i].to_string()).unwrap_or_else(|| s.to_string())
}
