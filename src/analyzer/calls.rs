use std::collections::HashMap;
use crate::model::FunctionInfo;

/// Simple regex-free call analysis: for each function body's core lines,
/// scan for known function names appearing as call targets.
pub fn analyze_calls(
    functions: &[FunctionInfo],
    source: &str,
) -> HashMap<String, Vec<String>> {
    let fn_names: Vec<&str> = functions.iter().map(|f| f.name.as_str()).collect();
    let mut result: HashMap<String, Vec<String>> = HashMap::new();

    // Split source into per-function sections using line ranges
    for func in functions {
        let (start, end) = func.line_range;
        let body_lines: Vec<&str> = source
            .lines()
            .enumerate()
            .filter(|(i, _)| *i + 1 >= start && *i + 1 <= end)
            .map(|(_, l)| l)
            .collect();

        let mut callees = Vec::new();
        for line in &body_lines {
            let trimmed = line.trim();
            for &name in &fn_names {
                if name == func.name.as_str() {
                    continue; // skip self
                }
                // Match `name(` pattern
                if contains_call(trimmed, name) && !callees.contains(&name.to_string()) {
                    callees.push(name.to_string());
                }
            }
        }

        result.insert(func.name.clone(), callees);
    }

    result
}

fn contains_call(line: &str, fn_name: &str) -> bool {
    // Look for `fn_name(` or `self.fn_name(` or `.fn_name(`
    let call_pattern = format!("{}(", fn_name);
    let method_pattern = format!(".{}(", fn_name);
    line.contains(&call_pattern) || line.contains(&method_pattern)
}
