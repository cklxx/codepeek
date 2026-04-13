pub mod function_list;
pub mod detail_view;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::model::{AppMode, AppState};
use function_list::render_function_list;

pub fn render(frame: &mut Frame, state: &AppState) {
    let size = frame.area();

    // Split vertically: main content + status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(2)])
        .split(size);

    let main_area = chunks[0];
    let status_area = chunks[1];

    render_function_list(frame, state, main_area);
    render_statusbar(frame, state, status_area);
}

fn render_statusbar(frame: &mut Frame, state: &AppState, area: Rect) {
    let file_name = state
        .file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    let total = state.visible_functions().len();
    let selected = if total > 0 { state.selected + 1 } else { 0 };

    let left = match &state.mode {
        AppMode::Search => format!("  Search: {}█", state.search_query),
        AppMode::Normal => format!("  codepeek · {}  [{}/{}]", file_name, selected, total),
    };

    let right = if state.status_msg.is_empty() {
        "  [j/k] nav  [Enter] expand  [/] search  [q] quit  ".to_string()
    } else {
        format!("  {}  ", state.status_msg)
    };

    let width = area.width as usize;
    let padding = width.saturating_sub(left.len() + right.len());
    let text = format!("{}{}{}", left, " ".repeat(padding), right);

    let bar = Paragraph::new(Line::from(vec![Span::styled(
        text,
        Style::default()
            .bg(Color::DarkGray)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )]));

    frame.render_widget(bar, area);
}
