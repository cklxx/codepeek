use anyhow::Result;
use std::path::PathBuf;
use tree_sitter::{Node, Parser};

use crate::model::FunctionInfo;

pub fn parse(source: &str, path: &PathBuf) -> Result<Vec<FunctionInfo>> {
    let _ = path;
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_python::LANGUAGE.into())
        .map_err(|e| anyhow::anyhow!("Failed to set Python language: {}", e))?;

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

fn collect_functions<'a>(
    node: Node<'a>,
    lines: &[&str],
    src: &[u8],
    class_owner: Option<&str>,
    out: &mut Vec<FunctionInfo>,
) {
    match node.kind() {
        "class_definition" => {
            let class_name = node
                .child_by_field_name("name")
                .and_then(|n| n.utf8_text(src).ok())
                .map(|s| s.to_string());

            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                collect_functions(child, lines, src, class_name.as_deref(), out);
            }
            return;
        }
        "function_definition" => {
            if let Some(mut info) = extract_function(node, lines, src) {
                info.owner = class_owner.map(|s| s.to_string());
                out.push(info);
            }
            // Don't recurse into function bodies
            return;
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_functions(child, lines, src, class_owner, out);
    }
}

fn extract_function(node: Node, lines: &[&str], src: &[u8]) -> Option<FunctionInfo> {
    let start = node.start_position().row;
    let end   = node.end_position().row;

    let name = node.child_by_field_name("name")?.utf8_text(src).ok()?.to_string();
    let signature = lines.get(start).unwrap_or(&"").trim().trim_end_matches(':').to_string();
    let doc = extract_docstring(node, src);

    let body: Vec<&str> = lines
        .get(start + 1..end)
        .unwrap_or(&[])
        .iter()
        .filter(|l| {
            let t = l.trim();
            !t.starts_with("\"\"\"") && !t.starts_with("'''")
        })
        .copied()
        .collect();

    Some(FunctionInfo::new(name, signature, (start + 1, end + 1), doc, &body))
}

fn extract_docstring(node: Node, src: &[u8]) -> Option<String> {
    let body = node.child_by_field_name("body")?;
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner = child.walk();
            for expr in child.children(&mut inner) {
                if expr.kind() == "string" {
                    let raw = expr.utf8_text(src).ok()?;
                    let cleaned = raw
                        .trim_matches(|c| c == '"' || c == '\'')
                        .trim()
                        .lines()
                        .next()
                        .unwrap_or("")
                        .trim()
                        .chars()
                        .take(120)
                        .collect();
                    return Some(cleaned);
                }
            }
        }
        break;
    }
    None
}
