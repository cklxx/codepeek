pub mod file_tree;
pub mod function_list;
pub mod highlight;

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
use highlight::tn;

pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Outer vertical split: content | statusbar
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(size);

    let content_area = rows[0];
    let status_area = rows[1];

    // Horizontal split: filetree (28 cols) | functions (rest)
    let tree_width = 28u16.min(size.width / 4);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(tree_width),
            Constraint::Min(30),
        ])
        .split(content_area);

    render_file_tree(frame, state, cols[0]);
    render_function_list(frame, state, cols[1]);
    render_statusbar(frame, state, status_area);
}

fn render_statusbar(frame: &mut Frame, state: &AppState, area: Rect) {
    let total = state.visible_fns().len();
    let sel = if total > 0 { state.fn_selected + 1 } else { 0 };

    let left = match &state.mode {
        AppMode::Search => format!("  / {}█", state.search_query),
        AppMode::Normal => {
            let panel = match state.focus {
                PanelFocus::FileTree     => "files",
                PanelFocus::FunctionList => "fns",
            };
            format!("  codepeek  [{}]  {}/{}", panel, sel, total)
        }
    };

    let right = if !state.status_msg.is_empty() {
        format!("  {}  ", state.status_msg)
    } else {
        "  j/k nav · Enter expand · / search · Tab switch · q quit  ".to_string()
    };

    let w = area.width as usize;
    let pad = w.saturating_sub(left.chars().count() + right.chars().count());
    let full = format!("{}{}{}", left, " ".repeat(pad), right);

    let bar = Paragraph::new(Line::from(vec![Span::styled(
        full,
        Style::default()
            .fg(tn::FG)
            .bg(tn::BG_DIM)
            .add_modifier(Modifier::BOLD),
    )]));
    frame.render_widget(bar, area);
}
