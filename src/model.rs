use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Unknown,
}

impl Language {
    pub fn from_path(path: &PathBuf) -> Self {
        match path.extension().and_then(|e| e.to_str()) {
            Some("rs") => Language::Rust,
            Some("py") => Language::Python,
            Some("js") => Language::JavaScript,
            Some("ts") | Some("tsx") => Language::TypeScript,
            _ => Language::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub signature: String,
    pub file_path: PathBuf,
    pub line_range: (usize, usize),
    pub summary: String,
    pub core_lines: Vec<String>,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
    pub is_expanded: bool,
}

impl FunctionInfo {
    pub fn new(
        name: String,
        signature: String,
        file_path: PathBuf,
        line_range: (usize, usize),
        raw_body: Vec<String>,
        doc_comment: Option<String>,
    ) -> Self {
        let summary = doc_comment.unwrap_or_else(|| {
            // Use first non-empty body line as summary
            raw_body
                .iter()
                .find(|l| !l.trim().is_empty())
                .cloned()
                .unwrap_or_default()
                .trim()
                .chars()
                .take(60)
                .collect()
        });

        let core_lines = extract_core_lines(&raw_body);

        FunctionInfo {
            name,
            signature,
            file_path,
            line_range,
            summary,
            core_lines,
            callers: vec![],
            callees: vec![],
            is_expanded: false,
        }
    }
}

/// Remove boilerplate: error handling, logging, imports, blank lines.
/// Keep: assignments, calls, returns, core conditions.
fn extract_core_lines(lines: &[String]) -> Vec<String> {
    let boilerplate_prefixes = [
        "log::", "tracing::", "debug!", "info!", "warn!", "error!",
        "println!", "eprintln!", "print!",
        "use ", "from ", "import ",
        "#[", "//",
    ];
    let error_patterns = [
        ".unwrap()", ".expect(", "Err(", "if err", "return Err",
        "map_err(", ".ok_or(", "bail!(", "ensure!(",
    ];

    let mut result: Vec<String> = Vec::new();
    let mut skipped_block = 0usize;

    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip boilerplate prefixes
        if boilerplate_prefixes.iter().any(|p| trimmed.starts_with(p)) {
            continue;
        }

        // Skip pure error-handling lines
        if error_patterns.iter().any(|p| trimmed.contains(p))
            && !trimmed.starts_with("let ")
            && !trimmed.starts_with("return")
        {
            skipped_block += 1;
            if skipped_block == 1 {
                result.push("    ...".to_string());
            }
            continue;
        }

        skipped_block = 0;

        // Trim indentation to 4 spaces max for display
        let indent = line.len() - line.trim_start().len();
        let display_indent = " ".repeat(indent.min(8));
        result.push(format!("{}{}", display_indent, trimmed));
    }

    // Limit to 10 core lines; collapse middle if longer
    if result.len() > 10 {
        let mut collapsed = result[..4].to_vec();
        collapsed.push("    ...".to_string());
        collapsed.extend_from_slice(&result[result.len() - 4..]);
        return collapsed;
    }

    result
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub functions: Vec<FunctionInfo>,
    pub selected: usize,
    pub file_path: PathBuf,
    pub scroll_offset: usize,
    pub search_query: String,
    pub mode: AppMode,
    pub status_msg: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
}

impl AppState {
    pub fn new(file_path: PathBuf, functions: Vec<FunctionInfo>) -> Self {
        AppState {
            functions,
            selected: 0,
            file_path,
            scroll_offset: 0,
            search_query: String::new(),
            mode: AppMode::Normal,
            status_msg: String::new(),
        }
    }

    pub fn visible_functions(&self) -> Vec<&FunctionInfo> {
        if self.search_query.is_empty() {
            self.functions.iter().collect()
        } else {
            let q = self.search_query.to_lowercase();
            self.functions
                .iter()
                .filter(|f| f.name.to_lowercase().contains(&q))
                .collect()
        }
    }

    pub fn select_next(&mut self) {
        let len = self.visible_functions().len();
        if len == 0 {
            return;
        }
        self.selected = (self.selected + 1).min(len - 1);
        self.ensure_visible();
    }

    pub fn select_prev(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.ensure_visible();
    }

    fn ensure_visible(&mut self) {
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        }
    }

    pub fn toggle_expand(&mut self) {
        // Map visible index back to global index
        let visible: Vec<usize> = if self.search_query.is_empty() {
            (0..self.functions.len()).collect()
        } else {
            let q = self.search_query.to_lowercase();
            self.functions
                .iter()
                .enumerate()
                .filter(|(_, f)| f.name.to_lowercase().contains(&q))
                .map(|(i, _)| i)
                .collect()
        };

        if let Some(&global_idx) = visible.get(self.selected) {
            self.functions[global_idx].is_expanded = !self.functions[global_idx].is_expanded;
        }
    }
}
