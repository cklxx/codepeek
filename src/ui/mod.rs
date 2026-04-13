pub mod file_tree;
pub mod function_list;
pub mod highlight;
pub mod source_view;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::model::{AppMode, AppState, PanelFocus};
use file_tree::render_file_tree;
use function_list::render_function_list;
use source_view::render_source_view;
use highlight::tn;

pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Outer: content | statusbar
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(size);

    let content = rows[0];
    let status  = rows[1];

    // Three columns: tree (fixed 22) | fn-list (30%) | source (rest)
    let tree_w = 22u16.min(size.width / 5);
    // If no functions and focus is SourceView, hide fn-list or collapse it
    let fn_w = if state.functions.is_empty() { 0u16 } else { (size.width.saturating_sub(tree_w)) * 30 / 100 };
    let src_w = size.width.saturating_sub(tree_w).saturating_sub(fn_w);

    let col_constraints = if state.functions.is_empty() {
        vec![Constraint::Length(tree_w), Constraint::Min(0)]
    } else {
        vec![
            Constraint::Length(tree_w),
            Constraint::Length(fn_w),
            Constraint::Min(0),
        ]
    };

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(col_constraints)
        .split(content);

    render_file_tree(frame, state, cols[0]);

    if state.functions.is_empty() {
        render_source_view(frame, state, cols[1]);
    } else {
        render_function_list(frame, state, cols[1]);
        render_source_view(frame, state, cols[2]);
    }

    render_statusbar(frame, state, status);
}

fn render_statusbar(frame: &mut Frame, state: &AppState, area: Rect) {
    let panel = match state.focus {
        PanelFocus::FileTree     => "files",
        PanelFocus::FunctionList => "fns",
        PanelFocus::SourceView   => "src",
    };

    let left = match &state.mode {
        AppMode::Search => format!("  /{}█", state.search_query),
        AppMode::Normal => {
            let total = state.visible_fns().len();
            let sel = if total > 0 { state.fn_selected + 1 } else { 0 };
            let fn_part = if total > 0 { format!("  {}/{}", sel, total) } else { String::new() };
            format!("  codepeek  [{}]{}  L{}",
                panel, fn_part,
                state.source_scroll + 1)
        }
    };

    let right = if !state.status_msg.is_empty() {
        format!("  {}  ", state.status_msg)
    } else {
        "  j/k nav · Tab switch panels · / search · q quit  ".to_string()
    };

    let w = area.width as usize;
    let lw = left.chars().count();
    let rw = right.chars().count();
    let pad = w.saturating_sub(lw + rw);
    let full = format!("{}{}{}", left, " ".repeat(pad), right);

    let bar = Paragraph::new(Line::from(Span::styled(
        full,
        Style::default().fg(tn::FG_DIM).bg(tn::BG_DIM).add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(bar, area);
}
