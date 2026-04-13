use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, Language, PanelFocus};
use super::highlight::{highlight_line, tn};

pub fn render_function_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FunctionList;
    let border_color = if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE };
    let lang = Language::from_path(&state.file_path);

    let file_name = state
        .file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?");

    let block = Block::default()
        .title(Line::from(vec![
            Span::raw(" "),
            Span::styled(file_name, Style::default().fg(tn::FUNC).add_modifier(Modifier::BOLD)),
            Span::styled(" — functions ", Style::default().fg(tn::FG_DIM)),
        ]))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = state.visible_fns();
    if visible.is_empty() {
        let msg = if state.search_query.is_empty() {
            "No functions found.".to_string()
        } else {
            format!("  No match for '{}'", state.search_query)
        };
        let p = Paragraph::new(msg)
            .style(Style::default().fg(tn::FG_DIM).bg(tn::BG));
        frame.render_widget(p, inner);
        return;
    }

    // Build all display lines
    let mut all_lines: Vec<Line> = Vec::new();
    // Track which visible-fn-index each display line belongs to (for scroll)
    let mut line_fn_idx: Vec<usize> = Vec::new();

    for (vi, func) in visible.iter().enumerate() {
        let is_sel = vi == state.fn_selected;
        let bg = if is_sel { tn::BG_HL } else { tn::BG };

        // ── Arrow + signature ──────────────────────────────────────────
        let arrow = if func.is_expanded { "▾" } else { "▸" };

        // Syntax-highlight the signature
        let sig_spans = highlight_signature(&func.signature, &lang);

        let mut header_spans = vec![
            Span::styled(
                format!(" {} ", arrow),
                Style::default()
                    .fg(if is_sel { tn::SELECTED_FG } else { tn::FG_DIM })
                    .bg(bg),
            ),
        ];
        for sp in &sig_spans {
            header_spans.push(Span::styled(
                sp.text.clone(),
                Style::default().fg(sp.color).bg(bg)
                    .add_modifier(if is_sel { Modifier::BOLD } else { Modifier::empty() }),
            ));
        }

        // Caller badge
        if !func.callers.is_empty() {
            header_spans.push(Span::styled(
                format!("  {} caller{}", func.callers.len(), if func.callers.len() == 1 { "" } else { "s" }),
                Style::default().fg(tn::BADGE_GREEN).bg(bg),
            ));
        }

        // Line range (right-aligned stub — full right-align is complex in ratatui inline)
        let w = inner.width as usize;
        let lr = format!(" L{}-{} ", func.line_range.0, func.line_range.1);
        if w > 20 {
            header_spans.push(Span::styled(lr, Style::default().fg(tn::FG_DARK).bg(bg)));
        }

        all_lines.push(Line::from(header_spans));
        line_fn_idx.push(vi);

        // ── Summary ────────────────────────────────────────────────────
        if !func.summary.is_empty() {
            let summary_text = truncate_str(&func.summary, inner.width as usize - 4);
            all_lines.push(Line::from(vec![
                Span::styled("     ", Style::default().bg(bg)),
                Span::styled(
                    format!("\"{}\"", summary_text),
                    Style::default()
                        .fg(tn::COMMENT)
                        .bg(bg)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]));
            line_fn_idx.push(vi);
        }

        // ── Expanded body ──────────────────────────────────────────────
        if func.is_expanded {
            let code_width = (inner.width as usize).saturating_sub(8);

            // Top border of code block
            all_lines.push(Line::from(vec![
                Span::styled("    ", Style::default().bg(tn::BG_DIM)),
                Span::styled(
                    format!("╭─ core ─{}╮", "─".repeat(code_width.saturating_sub(10))),
                    Style::default().fg(tn::BORDER_IDLE).bg(tn::BG_DIM),
                ),
            ]));
            line_fn_idx.push(vi);

            if func.core_lines.is_empty() {
                all_lines.push(Line::from(vec![
                    Span::styled("    │ ", Style::default().fg(tn::BORDER_IDLE).bg(tn::BG_DIM)),
                    Span::styled("(empty)", Style::default().fg(tn::FG_DIM).bg(tn::BG_DIM)),
                ]));
                line_fn_idx.push(vi);
            } else {
                for code_line in &func.core_lines {
                    let is_ellipsis = code_line.trim() == "...";
                    if is_ellipsis {
                        all_lines.push(Line::from(vec![
                            Span::styled("    │ ", Style::default().fg(tn::BORDER_IDLE).bg(tn::BG_DIM)),
                            Span::styled("  ···", Style::default().fg(tn::FG_DARK).bg(tn::BG_DIM)),
                        ]));
                    } else {
                        let mut spans = vec![
                            Span::styled("    │ ", Style::default().fg(tn::BORDER_IDLE).bg(tn::BG_DIM)),
                        ];
                        let hl = highlight_line(code_line, &lang);
                        let total_chars: usize = hl.iter().map(|s| s.text.chars().count()).sum();
                        let mut written = 0usize;
                        for hs in &hl {
                            let remaining = code_width.saturating_sub(written);
                            if remaining == 0 { break; }
                            let text = truncate_str(&hs.text, remaining);
                            written += text.chars().count();
                            spans.push(Span::styled(text, Style::default().fg(hs.color).bg(tn::BG_DIM)));
                        }
                        if total_chars == 0 {
                            spans.push(Span::raw(""));
                        }
                        all_lines.push(Line::from(spans));
                    }
                    line_fn_idx.push(vi);
                }
            }

            // Bottom border
            all_lines.push(Line::from(vec![
                Span::styled("    ", Style::default().bg(tn::BG_DIM)),
                Span::styled(
                    format!("╰{}╯", "─".repeat(code_width.saturating_sub(2))),
                    Style::default().fg(tn::BORDER_IDLE).bg(tn::BG_DIM),
                ),
            ]));
            line_fn_idx.push(vi);

            // Call graph row
            if !func.callees.is_empty() || !func.callers.is_empty() {
                let mut call_spans = vec![Span::styled("    ", Style::default().bg(bg))];
                if !func.callees.is_empty() {
                    call_spans.push(Span::styled("calls ", Style::default().fg(tn::FG_DIM).bg(bg)));
                    call_spans.push(Span::styled("→ ", Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
                    let names = truncate_str(&func.callees.join(", "), inner.width as usize / 2 - 4);
                    call_spans.push(Span::styled(names, Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
                    call_spans.push(Span::styled("    ", Style::default().bg(bg)));
                }
                if !func.callers.is_empty() {
                    call_spans.push(Span::styled("← ", Style::default().fg(tn::CALLER_COLOR).bg(bg)));
                    call_spans.push(Span::styled("called by ", Style::default().fg(tn::FG_DIM).bg(bg)));
                    let names = truncate_str(&func.callers.join(", "), inner.width as usize / 2 - 4);
                    call_spans.push(Span::styled(names, Style::default().fg(tn::CALLER_COLOR).bg(bg)));
                }
                all_lines.push(Line::from(call_spans));
                line_fn_idx.push(vi);
            }

            // Spacer after expanded fn
            all_lines.push(Line::from(Span::raw("")));
            line_fn_idx.push(vi);
        } else {
            // Collapsed: show inline call hints
            let mut hints: Vec<Span> = vec![Span::styled("   ", Style::default().bg(bg))];
            let has_hint = !func.callees.is_empty() || !func.callers.is_empty();
            if has_hint {
                if !func.callees.is_empty() {
                    hints.push(Span::styled("→ ", Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
                    let n = truncate_str(&func.callees.join(", "), 40);
                    hints.push(Span::styled(n, Style::default().fg(tn::FG_DARK).bg(bg)));
                    hints.push(Span::styled("   ", Style::default().bg(bg)));
                }
                if !func.callers.is_empty() {
                    hints.push(Span::styled("← ", Style::default().fg(tn::CALLER_COLOR).bg(bg)));
                    let n = truncate_str(&func.callers.join(", "), 40);
                    hints.push(Span::styled(n, Style::default().fg(tn::FG_DARK).bg(bg)));
                }
                all_lines.push(Line::from(hints));
                line_fn_idx.push(vi);
            }
            // Separator
            all_lines.push(Line::from(Span::styled(
                format!("   {}", "─".repeat((inner.width as usize).saturating_sub(6))),
                Style::default().fg(tn::BG_DIM),
            )));
            line_fn_idx.push(vi);
        }
    }

    // Scroll: keep selected function's first line in view
    let sel_start = line_fn_idx.iter().position(|&i| i == state.fn_selected).unwrap_or(0);
    let h = inner.height as usize;
    let scroll = if sel_start < h / 2 { 0 } else { sel_start.saturating_sub(h / 3) };

    let display: Vec<Line> = all_lines.into_iter().skip(scroll).take(h).collect();

    let p = Paragraph::new(display).style(Style::default().bg(tn::BG));
    frame.render_widget(p, inner);
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn truncate_str(s: &str, max: usize) -> String {
    if max < 4 {
        return s.chars().take(max).collect();
    }
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

/// Highlight just the signature line — keywords purple, fn name blue, types cyan.
fn highlight_signature(sig: &str, lang: &Language) -> Vec<super::highlight::Span> {
    highlight_line(sig, lang)
}
