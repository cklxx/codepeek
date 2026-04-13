use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, PanelFocus};
use super::highlight::tn;

pub fn render_file_tree(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FileTree;

    // Show project root name in title
    let root_name = state.tree_roots.first()
        .map(|n| n.name.as_str())
        .unwrap_or("files");

    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(root_name, Style::default().fg(tn::FG_MED)),
            Span::raw(" "),
        ]))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE }));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let flat = state.tree_flat();
    if flat.is_empty() {
        frame.render_widget(
            Paragraph::new("(empty)").style(Style::default().fg(tn::FG_DIM).bg(tn::BG)),
            inner,
        );
        return;
    }

    let h = inner.height as usize;
    let sel = state.tree_selected;
    // Scroll so selected is visible
    let scroll = if sel < h / 2 { 0 } else { sel.saturating_sub(h / 2) };

    let lines: Vec<Line> = flat
        .iter()
        .enumerate()
        .skip(scroll)
        .take(h)
        .map(|(idx, node)| {
            let is_sel = idx == sel;
            let is_active = !node.is_dir && node.path == state.file_path;
            let bg = if is_sel { tn::BG_SEL } else { tn::BG };

            // Indent based on depth (relative to tree root depth)
            let root_depth = state.tree_roots.first().map(|r| r.depth).unwrap_or(0);
            let rel_depth = node.depth.saturating_sub(root_depth);
            let indent = "  ".repeat(rel_depth);

            let (icon, icon_color) = if node.is_dir {
                if node.is_expanded { ("▾", tn::FG_MED) } else { ("▸", tn::FG_DIM) }
            } else {
                (" ", tn::FG_DARK)
            };

            let name_color = if is_sel {
                tn::SELECTED_FG
            } else if is_active {
                tn::NAME
            } else if node.is_dir {
                tn::FG_MED
            } else {
                tn::FG_DIM
            };

            Line::from(vec![
                Span::styled(
                    format!("{}{} ", indent, icon),
                    Style::default().fg(icon_color).bg(bg),
                ),
                Span::styled(
                    node.name.clone(),
                    Style::default()
                        .fg(name_color)
                        .bg(bg)
                        .add_modifier(if is_sel || is_active { Modifier::BOLD } else { Modifier::empty() }),
                ),
            ])
        })
        .collect();

    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(tn::BG)),
        inner,
    );
}
