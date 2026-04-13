use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set Python language: {}", e))?;

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
    if node.kind() == "function_definition" {
        if let Some(info) = extract_function(node, lines, source, path) {
            out.push(info);
        }
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

    let name_node = node.child_by_field_name("name")?;
    let name = name_node.utf8_text(source).ok()?.to_string();

    // Signature: the def ... : line
    let sig_line = lines.get(start_line).unwrap_or(&"").trim().to_string();
    let signature = sig_line.trim_end_matches(':').to_string();

    // Doc comment: first string literal in body
    let doc_comment = extract_docstring(node, source);

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

fn extract_docstring(node: Node, source: &[u8]) -> Option<String> {
    // Python docstrings are the first expression_statement containing a string
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner = child.walk();
            for expr in child.children(&mut inner) {
                if expr.kind() == "string" {
                    let raw = expr.utf8_text(source).ok()?;
                    let cleaned = raw
                        .trim_matches(|c| c == '"' || c == '\'' || c == '\n')
                        .trim()
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .chars()
                        .take(80)
                        .collect();
                    return Some(cleaned);
                }
            }
        }
        break; // only check first statement
    }
    None
}
