use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::AppState;

pub fn render_function_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let file_name = state
        .file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?");

    let block = Block::default()
        .title(format!(" codepeek · {} ", file_name))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = state.visible_functions();
    if visible.is_empty() {
        let msg = if state.search_query.is_empty() {
            "No functions found.".to_string()
        } else {
            format!("No functions matching '{}'.", state.search_query)
        };
        let p = Paragraph::new(msg).style(Style::default().fg(Color::DarkGray));
        frame.render_widget(p, inner);
        return;
    }

    // Build all lines, then scroll
    let mut all_lines: Vec<Line> = Vec::new();
    let mut line_to_fn: Vec<usize> = Vec::new(); // maps display line → fn index

    for (vis_idx, func) in visible.iter().enumerate() {
        let is_selected = vis_idx == state.selected;

        // ── Header row ──────────────────────────────────────────────
        let arrow = if func.is_expanded { "▾" } else { "▸" };

        let callers_badge = if !func.callers.is_empty() {
            format!(" [{} caller{}]", func.callers.len(), if func.callers.len() == 1 { "" } else { "s" })
        } else {
            String::new()
        };

        let line_info = format!(
            "  L{}-{}",
            func.line_range.0, func.line_range.1
        );

        let header_style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let bg = if is_selected { Color::DarkGray } else { Color::Reset };

        let header = Line::from(vec![
            Span::styled(format!(" {} ", arrow), Style::default().fg(Color::Cyan).bg(bg)),
            Span::styled(func.signature.clone(), header_style.bg(bg)),
            Span::styled(callers_badge, Style::default().fg(Color::Green).bg(bg)),
            Span::styled(line_info, Style::default().fg(Color::DarkGray).bg(bg)),
        ]);

        all_lines.push(header);
        line_to_fn.push(vis_idx);

        // ── Summary row ─────────────────────────────────────────────
        if !func.summary.is_empty() {
            let summary_style = if is_selected {
                Style::default().fg(Color::Gray).bg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            let summary = Line::from(vec![
                Span::raw("   "),
                Span::styled(
                    format!("\"{}\"", truncate(&func.summary, inner.width as usize - 6)),
                    summary_style,
                ),
            ]);
            all_lines.push(summary);
            line_to_fn.push(vis_idx);
        }

        // ── Expanded body ────────────────────────────────────────────
        if func.is_expanded {
            // Core logic box
            all_lines.push(Line::from(vec![
                Span::styled("   ┌─ core logic ", Style::default().fg(Color::Blue)),
                Span::styled(
                    "─".repeat((inner.width as usize).saturating_sub(18)),
                    Style::default().fg(Color::Blue),
                ),
                Span::styled("┐", Style::default().fg(Color::Blue)),
            ]));
            line_to_fn.push(vis_idx);

            if func.core_lines.is_empty() {
                all_lines.push(Line::from(vec![
                    Span::styled("   │ ", Style::default().fg(Color::Blue)),
                    Span::styled("(empty body)", Style::default().fg(Color::DarkGray)),
                ]));
                line_to_fn.push(vis_idx);
            } else {
                for code_line in &func.core_lines {
                    let display = truncate(code_line, inner.width as usize - 8);
                    all_lines.push(Line::from(vec![
                        Span::styled("   │ ", Style::default().fg(Color::Blue)),
                        Span::styled(display, Style::default().fg(Color::White)),
                    ]));
                    line_to_fn.push(vis_idx);
                }
            }

            all_lines.push(Line::from(vec![
                Span::styled("   └", Style::default().fg(Color::Blue)),
                Span::styled(
                    "─".repeat((inner.width as usize).saturating_sub(5)),
                    Style::default().fg(Color::Blue),
                ),
                Span::styled("┘", Style::default().fg(Color::Blue)),
            ]));
            line_to_fn.push(vis_idx);

            // Callees
            if !func.callees.is_empty() {
                let callees_str = func.callees.join(", ");
                all_lines.push(Line::from(vec![
                    Span::styled("   calls → ", Style::default().fg(Color::Magenta)),
                    Span::styled(
                        truncate(&callees_str, inner.width as usize - 14),
                        Style::default().fg(Color::White),
                    ),
                ]));
                line_to_fn.push(vis_idx);
            }

            // Callers
            if !func.callers.is_empty() {
                let callers_str = func.callers.join(", ");
                all_lines.push(Line::from(vec![
                    Span::styled("   called by ← ", Style::default().fg(Color::Green)),
                    Span::styled(
                        truncate(&callers_str, inner.width as usize - 18),
                        Style::default().fg(Color::White),
                    ),
                ]));
                line_to_fn.push(vis_idx);
            }

            // Separator
            all_lines.push(Line::from(Span::raw("")));
            line_to_fn.push(vis_idx);
        } else {
            // Compact callees/callers for collapsed view
            let mut call_info = Vec::new();
            if !func.callees.is_empty() {
                call_info.push(format!("→ {}", func.callees.join(", ")));
            }
            if !func.callers.is_empty() {
                call_info.push(format!("← {}", func.callers.join(", ")));
            }
            if !call_info.is_empty() {
                let text = call_info.join("  ");
                all_lines.push(Line::from(vec![
                    Span::raw("   "),
                    Span::styled(
                        truncate(&text, inner.width as usize - 6),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                line_to_fn.push(vis_idx);
            }

            // Blank line between functions
            all_lines.push(Line::from(Span::raw("")));
            line_to_fn.push(vis_idx);
        }
    }

    // Auto-scroll: find first line for selected function
    let selected_start = line_to_fn
        .iter()
        .position(|&fn_idx| fn_idx == state.selected)
        .unwrap_or(0);

    let scroll = state.scroll_offset;
    let visible_height = inner.height as usize;

    // Calculate scroll offset to keep selected in view
    let scroll_start = if selected_start < scroll {
        selected_start
    } else if selected_start >= scroll + visible_height {
        selected_start.saturating_sub(visible_height / 2)
    } else {
        scroll
    };

    let display_lines: Vec<Line> = all_lines
        .into_iter()
        .skip(scroll_start)
        .take(visible_height)
        .collect();

    let content = Paragraph::new(display_lines);
    frame.render_widget(content, inner);
}

fn truncate(s: &str, max: usize) -> String {
    if max < 4 {
        return s.chars().take(max).collect();
    }
    if s.chars().count() > max {
        format!("{}...", s.chars().take(max - 3).collect::<String>())
    } else {
        s.to_string()
    }
}
