use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, Language, PanelFocus};
use super::highlight::tn;

pub fn render_file_tree(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FileTree;
    let border_color = if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE };

    let block = Block::default()
        .title(Span::styled(" files ", Style::default().fg(tn::FG_DIM)))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let flat = state.tree_flat();
    if flat.is_empty() {
        let p = Paragraph::new("(empty)")
            .style(Style::default().fg(tn::FG_DIM));
        frame.render_widget(p, inner);
        return;
    }

    let height = inner.height as usize;
    let selected = state.tree_selected;

    // Scroll so selected is always visible
    let scroll = if selected < height / 2 {
        0
    } else {
        (selected + 1).saturating_sub(height)
    };

    let lines: Vec<Line> = flat
        .iter()
        .enumerate()
        .skip(scroll)
        .take(height)
        .map(|(idx, node)| {
            let is_sel = idx == selected;
            let indent = "  ".repeat(node.depth);

            let (icon, icon_color) = if node.is_dir {
                if node.is_expanded { ("▾ ", tn::KEYWORD) } else { ("▸ ", tn::FG_DIM) }
            } else {
                let c = match Language::from_path(&node.path) {
                    Language::Rust       => tn::NUMBER,
                    Language::Python     => tn::STRING,
                    Language::JavaScript => tn::LIFETIME,
                    Language::TypeScript => tn::TYPE_,
                    Language::Unknown    => tn::FG_DIM,
                };
                ("  ", c)
            };

            // Highlight active file
            let is_active = !node.is_dir && node.path == state.file_path;

            let name_style = if is_sel {
                Style::default()
                    .fg(tn::SELECTED_FG)
                    .bg(tn::BG_HL)
                    .add_modifier(Modifier::BOLD)
            } else if is_active {
                Style::default().fg(tn::FUNC).add_modifier(Modifier::BOLD)
            } else if node.is_dir {
                Style::default().fg(tn::FG)
            } else {
                Style::default().fg(tn::FG_DIM)
            };

            Line::from(vec![
                Span::styled(format!("{}{}", indent, icon), Style::default().fg(icon_color).bg(if is_sel { tn::BG_HL } else { tn::BG })),
                Span::styled(node.name.clone(), name_style.bg(if is_sel { tn::BG_HL } else { tn::BG })),
            ])
        })
        .collect();

    let p = Paragraph::new(lines)
        .style(Style::default().bg(tn::BG));
    frame.render_widget(p, inner);
}
