use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set JS language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    let root = tree.root_node();
    let lines: Vec<&str> = source.lines().collect();
    let source_bytes = source.as_bytes();

    let mut functions = Vec::new();
    collect_functions(root, &lines, source_bytes, path, &mut functions);

    Ok(functions)
}

fn collect_functions(
    node: Node,
    lines: &[&str],
    source: &[u8],
    path: &PathBuf,
    out: &mut Vec<FunctionInfo>,
) {
    match node.kind() {
        "function_declaration"
        | "function"
        | "arrow_function"
        | "method_definition" => {
            if let Some(info) = extract_function(node, lines, source, path) {
                out.push(info);
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, lines, source, path, out);
    }
}

fn extract_function(
    node: Node,
    lines: &[&str],
    source: &[u8],
    path: &PathBuf,
) -> Option<FunctionInfo> {
    let start_line = node.start_position().row;
    let end_line = node.end_position().row;

    // Try name field first, then look for variable declarator parent
    let name = if let Some(name_node) = node.child_by_field_name("name") {
        name_node.utf8_text(source).ok()?.to_string()
    } else {
        // Anonymous function in variable declarator: `const foo = () => {}`
        // Walk up isn't available easily, so generate placeholder
        format!("<anonymous>@{}", start_line + 1)
    };

    // Signature: first line up to opening brace or arrow
    let sig_raw = lines.get(start_line).unwrap_or(&"").trim();
    let signature = if let Some(pos) = sig_raw.find('{') {
        sig_raw[..pos].trim_end().to_string()
    } else {
        sig_raw.to_string()
    };

    // Doc comment from preceding JSDoc
    let doc_comment = extract_jsdoc(start_line, lines);

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
        (start_line + 1, end_line + 1),
        raw_body,
        doc_comment,
    ))
}

fn extract_jsdoc(fn_start: usize, lines: &[&str]) -> Option<String> {
    if fn_start == 0 {
        return None;
    }

    // Look for /** ... */ block ending just before this function
    let mut i = fn_start as isize - 1;

    // Skip blank lines and decorators
    while i >= 0 && (lines[i as usize].trim().is_empty() || lines[i as usize].trim().starts_with('@')) {
        i -= 1;
    }

    if i < 0 {
        return None;
    }

    let line = lines[i as usize].trim();
    if line.ends_with("*/") {
        // Scan backward for /**
        while i >= 0 {
            let l = lines[i as usize].trim();
            if l.starts_with("/**") || l.starts_with("/*") {
                // Extract first descriptive line
                let desc: String = lines
                    .get((i as usize)..(fn_start))
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|l| {
                        let t = l.trim().trim_start_matches('*').trim();
                        if t.is_empty() || t.starts_with('/') || t.starts_with('@') {
                            None
                        } else {
                            Some(t.to_string())
                        }
                    })
                    .next()
                    .unwrap_or_default();
                return if desc.is_empty() { None } else { Some(desc.chars().take(80).collect()) };
            }
            i -= 1;
        }
    } else if line.starts_with("//") {
        return Some(line.trim_start_matches('/').trim().chars().take(80).collect());
    }

    None
}
