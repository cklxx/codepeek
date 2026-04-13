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
    let border_color = if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE };
    let lang = Language::from_path(&state.file_path);

    // Title: file name + function context
    let ctx = state.selected_fn()
        .map(|f| format!(" fn {} ", f.name))
        .unwrap_or_default();
    let file_name = state.file_path.file_name()
        .and_then(|n| n.to_str()).unwrap_or("?");

    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(file_name, Style::default().fg(tn::FG_DIM)),
            Span::styled(ctx, Style::default().fg(tn::FUNC).add_modifier(Modifier::BOLD)),
        ]))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if state.source_lines.is_empty() {
        let p = Paragraph::new("(empty file)")
            .style(Style::default().fg(tn::FG_DIM).bg(tn::BG));
        frame.render_widget(p, inner);
        return;
    }

    // Determine highlighted range from selected function
    let highlight_range: Option<(usize, usize)> = state.selected_fn()
        .map(|f| (f.line_range.0, f.line_range.1)); // 1-indexed inclusive

    let height = inner.height as usize;
    let total_lines = state.source_lines.len();
    let scroll = state.source_scroll.min(total_lines.saturating_sub(1));

    // Gutter width: enough for line numbers
    let gutter_w = digits(total_lines) + 1;

    let lines: Vec<Line> = state.source_lines
        .iter()
        .enumerate()
        .skip(scroll)
        .take(height)
        .map(|(i, raw_line)| {
            let line_no = i + 1; // 1-indexed
            let in_fn = highlight_range
                .map(|(s, e)| line_no >= s && line_no <= e)
                .unwrap_or(false);
            let is_fn_first = highlight_range.map(|(s, _)| line_no == s).unwrap_or(false);

            let bg = if in_fn { tn::BG_DIM } else { tn::BG };

            // Gutter: line number
            let gutter_style = Style::default()
                .fg(if in_fn { tn::FG_DARK } else { tn::BG_DIM })
                .bg(tn::BG);
            let gutter_text = if is_fn_first {
                format!("{:>width$}▶ ", line_no, width = gutter_w - 1)
            } else {
                format!("{:>width$}  ", line_no, width = gutter_w)
            };

            // Code spans with syntax highlighting
            let code_spans = highlight_line(raw_line, &lang);
            let max_code_w = (inner.width as usize).saturating_sub(gutter_w + 2);

            let mut spans = vec![
                Span::styled(gutter_text, gutter_style),
            ];

            // Highlight the fn-start line marker
            if is_fn_first {
                spans[0] = Span::styled(
                    spans[0].content.clone().into_owned(),
                    Style::default().fg(tn::FUNC).bg(tn::BG),
                );
            }

            let mut col = 0usize;
            for cs in &code_spans {
                if col >= max_code_w { break; }
                let text = truncate_str(&cs.text, max_code_w - col);
                col += text.chars().count();
                spans.push(Span::styled(text, Style::default().fg(cs.color).bg(bg)));
            }

            Line::from(spans)
        })
        .collect();

    // Scroll indicator
    let pct = if total_lines <= height { 100 } else {
        (scroll * 100) / (total_lines - height)
    };
    let _ = pct; // used below in status, skip for now

    let p = Paragraph::new(lines).style(Style::default().bg(tn::BG));
    frame.render_widget(p, inner);
}

fn digits(n: usize) -> usize {
    if n == 0 { return 1; }
    let mut d = 0;
    let mut x = n;
    while x > 0 { d += 1; x /= 10; }
    d
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.chars().count() <= max { s.to_string() }
    else { s.chars().take(max).collect() }
}
