use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let _ = path;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_javascript::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set JS language: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    let root = tree.root_node();
    let lines: Vec<&str> = source.lines().collect();
    let src = source.as_bytes();

    let mut functions = Vec::new();
    collect_functions(root, &lines, src, &mut functions);
    Ok(functions)
}

fn collect_functions(node: Node, lines: &[&str], src: &[u8], out: &mut Vec<FunctionInfo>) {
    match node.kind() {
        "function_declaration" | "function" | "arrow_function" | "method_definition" => {
            if let Some(info) = extract_function(node, lines, src) {
                out.push(info);
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, lines, src, out);
    }
}

fn extract_function(node: Node, lines: &[&str], src: &[u8]) -> Option<FunctionInfo> {
    let start = node.start_position().row;
    let end = node.end_position().row;

    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(src).ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("<anon>@{}", start + 1));

    let sig_raw = lines.get(start).unwrap_or(&"").trim();
    let signature = sig_raw
        .find('{')
        .map(|p| sig_raw[..p].trim_end().to_string())
        .unwrap_or_else(|| sig_raw.to_string());

    let doc = extract_jsdoc(start, lines);
    let first_body = lines
        .get(start + 1..end)
        .unwrap_or(&[])
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string());

    Some(FunctionInfo::new(name, signature, (start + 1, end + 1), doc, first_body))
}

fn extract_jsdoc(fn_start: usize, lines: &[&str]) -> Option<String> {
    if fn_start == 0 { return None; }
    let mut i = fn_start as isize - 1;
    while i >= 0 && lines[i as usize].trim().is_empty() { i -= 1; }
    if i < 0 { return None; }
    let line = lines[i as usize].trim();
    if line.ends_with("*/") {
        while i >= 0 {
            let l = lines[i as usize].trim();
            if l.starts_with("/**") || l.starts_with("/*") {
                let desc: String = lines
                    .get(i as usize..fn_start)
                    .unwrap_or(&[])
                    .iter()
                    .filter_map(|l| {
                        let t = l.trim().trim_start_matches('*').trim();
                        if t.is_empty() || t.starts_with('/') || t.starts_with('@') { None }
                        else { Some(t.to_string()) }
                    })
                    .next()
                    .unwrap_or_default();
                return if desc.is_empty() { None } else { Some(desc.chars().take(100).collect()) };
            }
            i -= 1;
        }
    } else if line.starts_with("//") {
        return Some(line.trim_start_matches('/').trim().chars().take(100).collect());
    }
    None
}
