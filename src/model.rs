use std::path::PathBuf;

// ─── Language ────────────────────────────────────────────────────────────────

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

    pub fn is_supported(path: &PathBuf) -> bool {
        !matches!(Language::from_path(path), Language::Unknown)
    }
}

// ─── FunctionInfo ────────────────────────────────────────────────────────────

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
            raw_body
                .iter()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.trim().chars().take(72).collect())
                .unwrap_or_default()
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

/// Strip boilerplate: keep assignments, calls, returns, core conditions.
fn extract_core_lines(lines: &[String]) -> Vec<String> {
    let log_prefixes = [
        "log::", "tracing::", "debug!", "info!", "warn!", "error!",
        "println!", "eprintln!", "print!", "trace!",
    ];
    let import_prefixes = ["use ", "from ", "import "];
    let attr_prefixes = ["#[", "#!["];
    let error_patterns = [
        ".unwrap()", ".expect(", "bail!(", "ensure!(",
        "map_err(", ".ok_or(",
    ];

    let mut result: Vec<String> = Vec::new();
    let mut suppressed = false;

    for line in lines {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }

        if log_prefixes.iter().any(|p| t.starts_with(p))
            || import_prefixes.iter().any(|p| t.starts_with(p))
            || attr_prefixes.iter().any(|p| t.starts_with(p))
            || t.starts_with("//")
        {
            continue;
        }

        // Pure error-handling lines (not assignments)
        let is_err = error_patterns.iter().any(|p| t.contains(p))
            && !t.starts_with("let ")
            && !t.starts_with("return");
        if is_err {
            if !suppressed {
                result.push("    ...".to_string());
                suppressed = true;
            }
            continue;
        }
        suppressed = false;

        let indent = line.len() - line.trim_start().len();
        let di = " ".repeat(indent.min(12));
        result.push(format!("{}{}", di, t));
    }

    // Collapse middle if > 12 lines
    if result.len() > 12 {
        let mut out = result[..5].to_vec();
        out.push("    ...".to_string());
        out.extend_from_slice(&result[result.len() - 5..]);
        return out;
    }

    result
}

// ─── FileTree ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_expanded: bool,
    pub depth: usize,
    pub children: Vec<FileNode>,
}

impl FileNode {
    pub fn load(path: PathBuf, depth: usize) -> Option<Self> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();

        if path.is_dir() {
            // Skip hidden dirs and common noise
            if name.starts_with('.') || name == "target" || name == "node_modules" || name == "__pycache__" {
                return None;
            }
            let mut children: Vec<FileNode> = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&path) {
                let mut paths: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .collect();
                paths.sort();
                // Dirs first, then files
                paths.sort_by(|a, b| b.is_dir().cmp(&a.is_dir()));
                for p in paths {
                    if let Some(child) = FileNode::load(p, depth + 1) {
                        children.push(child);
                    }
                }
            }
            if children.is_empty() {
                return None;
            }
            Some(FileNode { name, path, is_dir: true, is_expanded: depth == 0, depth, children })
        } else {
            // Only show supported source files
            if !Language::is_supported(&path) {
                return None;
            }
            Some(FileNode { name, path, is_dir: false, is_expanded: false, depth, children: vec![] })
        }
    }

    /// Flatten the visible tree into a list for rendering/navigation.
    pub fn flatten(&self) -> Vec<&FileNode> {
        let mut out = vec![self as &FileNode];
        if self.is_dir && self.is_expanded {
            for child in &self.children {
                out.extend(child.flatten());
            }
        }
        out
    }

    /// Flatten mutably for toggle operations.
    pub fn flatten_mut(nodes: &mut Vec<FileNode>) -> Vec<*mut FileNode> {
        let mut out = Vec::new();
        for node in nodes.iter_mut() {
            out.push(node as *mut FileNode);
            if node.is_dir && node.is_expanded {
                out.extend(FileNode::flatten_mut(&mut node.children));
            }
        }
        out
    }
}

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PanelFocus {
    FileTree,
    FunctionList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
}

#[derive(Debug, Clone)]
pub struct AppState {
    // Function panel
    pub functions: Vec<FunctionInfo>,
    pub fn_selected: usize,
    pub fn_scroll: usize,
    pub file_path: PathBuf,

    // File tree panel
    pub tree_roots: Vec<FileNode>,
    pub tree_selected: usize,
    pub tree_root_path: PathBuf,

    // Global
    pub focus: PanelFocus,
    pub search_query: String,
    pub mode: AppMode,
    pub status_msg: String,
}

impl AppState {
    pub fn new(
        file_path: PathBuf,
        functions: Vec<FunctionInfo>,
        root_dir: PathBuf,
    ) -> Self {
        let tree_roots = build_tree(&root_dir);

        AppState {
            functions,
            fn_selected: 0,
            fn_scroll: 0,
            file_path,
            tree_roots,
            tree_selected: 0,
            tree_root_path: root_dir,
            focus: PanelFocus::FunctionList,
            search_query: String::new(),
            mode: AppMode::Normal,
            status_msg: String::new(),
        }
    }

    // ── Function list helpers ──────────────────────────────────────────────

    pub fn visible_fns(&self) -> Vec<&FunctionInfo> {
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

    pub fn fn_select_next(&mut self) {
        let len = self.visible_fns().len();
        if len > 0 {
            self.fn_selected = (self.fn_selected + 1).min(len - 1);
        }
    }

    pub fn fn_select_prev(&mut self) {
        if self.fn_selected > 0 {
            self.fn_selected -= 1;
        }
    }

    pub fn fn_toggle_expand(&mut self) {
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
        if let Some(&gi) = visible.get(self.fn_selected) {
            self.functions[gi].is_expanded = !self.functions[gi].is_expanded;
        }
    }

    // ── Tree helpers ───────────────────────────────────────────────────────

    pub fn tree_flat(&self) -> Vec<&FileNode> {
        let mut out = Vec::new();
        for root in &self.tree_roots {
            out.extend(root.flatten());
        }
        out
    }

    pub fn tree_select_next(&mut self) {
        let len = self.tree_flat().len();
        if len > 0 {
            self.tree_selected = (self.tree_selected + 1).min(len - 1);
        }
    }

    pub fn tree_select_prev(&mut self) {
        if self.tree_selected > 0 {
            self.tree_selected -= 1;
        }
    }

    /// Toggle directory expand/collapse, or return file path to open.
    pub fn tree_activate(&mut self) -> Option<PathBuf> {
        // First collect what we need (avoids borrow conflict)
        let (is_dir, path) = {
            let flat = self.tree_flat();
            let node = flat.get(self.tree_selected)?;
            (node.is_dir, node.path.clone())
        };
        if is_dir {
            toggle_node_by_path(&mut self.tree_roots, &path);
            None
        } else {
            Some(path)
        }
    }
}

fn toggle_node_by_path(nodes: &mut Vec<FileNode>, target: &PathBuf) {
    for node in nodes.iter_mut() {
        if &node.path == target {
            node.is_expanded = !node.is_expanded;
            return;
        }
        if node.is_dir {
            toggle_node_by_path(&mut node.children, target);
        }
    }
}

fn build_tree(root: &PathBuf) -> Vec<FileNode> {
    if root.is_file() {
        // Single-file mode: build a minimal tree showing the parent dir
        let parent = root.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| root.clone());
        if let Some(node) = FileNode::load(parent, 0) {
            return vec![node];
        }
        return vec![];
    }
    if let Some(node) = FileNode::load(root.clone(), 0) {
        vec![node]
    } else {
        vec![]
    }
}
