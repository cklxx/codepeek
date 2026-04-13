use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, Language, PanelFocus};
use super::highlight::{highlight_line, tn};

pub fn render_source_view(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::SourceView;
    let lang = Language::from_path(&state.file_path);
    let file_name = state.file_path.file_name()
        .and_then(|n| n.to_str()).unwrap_or("?");

    // Title: filename  Owner::fn_name  (owner in amber, fn in blue — same contrast hierarchy as fn list)
    let mut title_spans = vec![
        Span::raw(" "),
        Span::styled(file_name, Style::default().fg(tn::FG_MED)),
    ];
    if let Some(f) = state.selected_fn() {
        title_spans.push(Span::styled("  ", Style::default()));
        if let Some(owner) = &f.owner {
            title_spans.push(Span::styled(
                owner.clone(),
                Style::default().fg(tn::OWNER).add_modifier(Modifier::BOLD),
            ));
            title_spans.push(Span::styled("::", Style::default().fg(tn::FG_DARK)));
        }
        title_spans.push(Span::styled(
            f.name.clone(),
            Style::default().fg(tn::NAME).add_modifier(Modifier::BOLD),
        ));
        title_spans.push(Span::raw("  "));
    }

    let block = Block::default()
        .title(Line::from(title_spans))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(
            if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE }
        ));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.source_lines.is_empty() {
        frame.render_widget(
            Paragraph::new("(empty)").style(Style::default().fg(tn::FG_DIM).bg(tn::BG)),
            inner,
        );
        return;
    }

    let height = inner.height as usize;
    let total = state.source_lines.len();
    let scroll = state.source_scroll.min(total.saturating_sub(1));
    let gutter_w = digits(total) + 1; // e.g. "  42 "

    // Highlighted range from selected function
    let hl_range = state.selected_fn().map(|f| f.line_range); // 1-indexed inclusive

    let lines: Vec<Line> = state.source_lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(height)
        .map(|(idx, raw)| {
            let line_no = idx + 1; // 1-indexed
            let in_fn = hl_range.map(|(s, e)| line_no >= s && line_no <= e).unwrap_or(false);
            let is_start = hl_range.map(|(s, _)| line_no == s).unwrap_or(false);
            let bg = if in_fn { tn::BG_DIM } else { tn::BG };

            // ── Gutter — ghost level except fn-start marker ───────────
            let (gutter_str, gutter_color) = if is_start {
                (format!("{:>w$}▶ ", line_no, w = gutter_w), tn::NAME)
            } else if in_fn {
                (format!("{:>w$}  ", line_no, w = gutter_w), tn::FG_DARK) // slightly brighter inside fn
            } else {
                (format!("{:>w$}  ", line_no, w = gutter_w), tn::LINENUM)
            };

            let mut spans = vec![
                Span::styled(gutter_str, Style::default().fg(gutter_color).bg(tn::BG)),
            ];

            // ── Code ─────────────────────────────────────────────────
            let max_w = (inner.width as usize).saturating_sub(gutter_w + 2);
            let hl = highlight_line(raw, &lang);

            let mut col = 0usize;
            for hs in &hl {
                if col >= max_w { break; }
                let text = take_chars(&hs.text, max_w - col);
                col += text.chars().count();
                spans.push(Span::styled(text, Style::default().fg(hs.color).bg(bg)));
            }
            if hl.is_empty() {
                // blank line — just fill bg
                spans.push(Span::styled(" ", Style::default().bg(bg)));
            }

            Line::from(spans)
        })
        .collect();

    // Scroll position indicator in bottom-right of block (drawn as part of border title)
    // Just render the content — simple and clean
    frame.render_widget(
        Paragraph::new(lines).style(Style::default().bg(tn::BG)),
        inner,
    );
}

fn digits(n: usize) -> usize {
    if n < 10 { 1 } else if n < 100 { 2 } else if n < 1000 { 3 } else { 4 }
}

fn take_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}
