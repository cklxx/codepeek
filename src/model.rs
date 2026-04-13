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
    pub line_range: (usize, usize), // 1-indexed, inclusive
    pub summary: String,
    pub callers: Vec<String>,
    pub callees: Vec<String>,
}

impl FunctionInfo {
    pub fn new(
        name: String,
        signature: String,
        line_range: (usize, usize),
        doc_comment: Option<String>,
        first_body_line: Option<String>,
    ) -> Self {
        let summary = doc_comment
            .or(first_body_line)
            .unwrap_or_default()
            .trim()
            .chars()
            .take(80)
            .collect();

        FunctionInfo {
            name,
            signature,
            line_range,
            summary,
            callers: vec![],
            callees: vec![],
        }
    }
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
        let name = path.file_name()?.to_str()?.to_string();

        if path.is_dir() {
            if name.starts_with('.')
                || matches!(name.as_str(), "target" | "node_modules" | "__pycache__" | ".git")
            {
                return None;
            }
            let mut children: Vec<FileNode> = std::fs::read_dir(&path)
                .ok()?
                .filter_map(|e| e.ok())
                .filter_map(|e| FileNode::load(e.path(), depth + 1))
                .collect();
            if children.is_empty() {
                return None;
            }
            // dirs first, then alphabetical
            children.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
            Some(FileNode {
                name,
                path,
                is_dir: true,
                is_expanded: depth == 0,
                depth,
                children,
            })
        } else {
            // Show all text-ish files, not just supported ones
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let ok = matches!(
                ext,
                "rs" | "py" | "js" | "ts" | "tsx" | "go" | "c" | "cpp" | "h"
                    | "java" | "kt" | "swift" | "rb" | "sh" | "toml" | "yaml"
                    | "yml" | "json" | "md" | "txt"
            );
            if !ok {
                return None;
            }
            Some(FileNode {
                name,
                path,
                is_dir: false,
                is_expanded: false,
                depth,
                children: vec![],
            })
        }
    }

    pub fn flatten(&self) -> Vec<&FileNode> {
        let mut out = vec![self as &FileNode];
        if self.is_dir && self.is_expanded {
            for child in &self.children {
                out.extend(child.flatten());
            }
        }
        out
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
    let dir = if root.is_file() {
        root.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| root.clone())
    } else {
        root.clone()
    };
    FileNode::load(dir, 0).map(|n| vec![n]).unwrap_or_default()
}

// ─── Panel focus ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum PanelFocus {
    FileTree,
    FunctionList,
    SourceView,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Normal,
    Search,
}

// ─── App state ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct AppState {
    // File being viewed
    pub file_path: PathBuf,
    pub source_lines: Vec<String>,

    // Function list
    pub functions: Vec<FunctionInfo>,
    pub fn_selected: usize,

    // Source view scroll
    pub source_scroll: usize,

    // File tree
    pub tree_roots: Vec<FileNode>,
    pub tree_selected: usize,

    // Global
    pub focus: PanelFocus,
    pub search_query: String,
    pub mode: AppMode,
    pub status_msg: String,
}

impl AppState {
    pub fn new(file_path: PathBuf, functions: Vec<FunctionInfo>, root_dir: PathBuf) -> Self {
        let source_lines = read_source_lines(&file_path);
        let tree_roots = build_tree(&root_dir);
        let mut s = AppState {
            file_path,
            source_lines,
            functions,
            fn_selected: 0,
            source_scroll: 0,
            tree_roots,
            tree_selected: 0,
            focus: PanelFocus::FunctionList,
            search_query: String::new(),
            mode: AppMode::Normal,
            status_msg: String::new(),
        };
        s.sync_source_scroll();
        s
    }

    // ── visible functions ──────────────────────────────────────────────────

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

    pub fn selected_fn(&self) -> Option<&FunctionInfo> {
        self.visible_fns().into_iter().nth(self.fn_selected)
    }

    // ── fn list navigation ─────────────────────────────────────────────────

    pub fn fn_select_next(&mut self) {
        let len = self.visible_fns().len();
        if len > 0 && self.fn_selected + 1 < len {
            self.fn_selected += 1;
            self.sync_source_scroll();
        }
    }

    pub fn fn_select_prev(&mut self) {
        if self.fn_selected > 0 {
            self.fn_selected -= 1;
            self.sync_source_scroll();
        }
    }

    /// Scroll source view to show the currently selected function.
    pub fn sync_source_scroll(&mut self) {
        if let Some(f) = self.selected_fn() {
            let start = f.line_range.0.saturating_sub(1); // 0-indexed
            self.source_scroll = start.saturating_sub(3); // a little context above
        }
    }

    // ── source view navigation ─────────────────────────────────────────────

    pub fn source_scroll_down(&mut self, n: usize) {
        let max = self.source_lines.len().saturating_sub(1);
        self.source_scroll = (self.source_scroll + n).min(max);
    }

    pub fn source_scroll_up(&mut self, n: usize) {
        self.source_scroll = self.source_scroll.saturating_sub(n);
    }

    // ── tree navigation ────────────────────────────────────────────────────

    pub fn tree_flat(&self) -> Vec<&FileNode> {
        self.tree_roots.iter().flat_map(|r| r.flatten()).collect()
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

    pub fn tree_activate(&mut self) -> Option<PathBuf> {
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

    // ── load a new file ────────────────────────────────────────────────────

    pub fn load_file(&mut self, path: PathBuf, functions: Vec<FunctionInfo>) {
        self.file_path = path;
        self.source_lines = read_source_lines(&self.file_path);
        self.functions = functions;
        self.fn_selected = 0;
        self.source_scroll = 0;
        self.search_query.clear();
        self.status_msg.clear();
        self.focus = if self.functions.is_empty() {
            PanelFocus::SourceView
        } else {
            PanelFocus::FunctionList
        };
        self.sync_source_scroll();
    }
}

pub fn read_source_lines(path: &PathBuf) -> Vec<String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect()
}
