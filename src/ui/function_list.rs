/// Function list panel.
///
/// Visual hierarchy (based on eye-tracking research):
///   PRIMARY   — function name: bold, bright blue, leftmost position
///   SECONDARY — return type: cyan, right of name
///   TERTIARY  — `pub fn` keywords + params: muted purple, second row
///   SUPPORT   — summary (docstring): italic, comment color
///   PERIPHERAL — caller badge, line number: low-contrast, right-aligned
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, Language, PanelFocus};
use super::highlight::tn;

pub fn render_function_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FunctionList;
    let border_color = if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE };

    let count = state.visible_fns().len();
    let total = state.functions.len();
    let count_str = if state.search_query.is_empty() {
        format!(" {} fns ", total)
    } else {
        format!(" {}/{} ", count, total)
    };

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" functions ", Style::default().fg(tn::FG_DIM)),
            Span::styled(count_str, Style::default().fg(tn::FG_DARK)),
        ]))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(border_color));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = state.visible_fns();

    if visible.is_empty() {
        let msg = if state.search_query.is_empty() {
            " no functions found — view source on the right".to_string()
        } else {
            format!(" no match for '{}'", state.search_query)
        };
        let p = Paragraph::new(msg).style(Style::default().fg(tn::FG_DARK).bg(tn::BG));
        frame.render_widget(p, inner);
        return;
    }

    let w = inner.width as usize;
    let h = inner.height as usize;

    // Build all display lines
    let mut all_lines: Vec<Line> = Vec::new();
    let mut line_fn_idx: Vec<usize> = Vec::new(); // which fn each display line belongs to

    for (vi, func) in visible.iter().enumerate() {
        let is_sel = vi == state.fn_selected;
        let bg = if is_sel { tn::BG_HL } else { tn::BG };

        // ── Decompose signature ────────────────────────────────────────────
        let (vis_kw, fn_name, params, ret_type) = decompose_sig(&func.signature);
        let lang = Language::from_path(&state.file_path);
        let _ = lang;

        // ── ROW 1: [▸] FnName  → RetType   ↑N  L123 ─────────────────────
        let arrow = if is_sel { "▶" } else { "·" };
        let arrow_color = if is_sel { tn::SELECTED_FG } else { tn::FG_DARK };

        let caller_badge = if !func.callers.is_empty() {
            format!(" ↑{}", func.callers.len())
        } else {
            String::new()
        };
        let callee_badge = if !func.callees.is_empty() {
            format!(" ↓{}", func.callees.len())
        } else {
            String::new()
        };
        let line_badge = format!(" L{}", func.line_range.0);

        // Right-side badges width
        let badges = format!("{}{}{}", caller_badge, callee_badge, line_badge);
        let name_ret_max = w.saturating_sub(3 + badges.len() + 2);

        let name_display = truncate(&fn_name, name_ret_max.saturating_sub(ret_type.len() + 4));
        let ret_display = if !ret_type.is_empty() {
            truncate(&ret_type, 20)
        } else {
            String::new()
        };

        let mut row1 = vec![
            Span::styled(format!(" {} ", arrow), Style::default().fg(arrow_color).bg(bg)),
            // PRIMARY: function name — bold, bright, leftmost
            Span::styled(
                name_display,
                Style::default()
                    .fg(if is_sel { tn::SELECTED_FG } else { tn::FUNC })
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];

        if !ret_display.is_empty() {
            row1.push(Span::styled(" → ", Style::default().fg(tn::FG_DARK).bg(bg)));
            // SECONDARY: return type
            row1.push(Span::styled(
                ret_display.clone(),
                Style::default().fg(tn::TYPE_).bg(bg),
            ));
        }

        // Spacer + peripheral badges (right-aligned approximation)
        let ret_display_len = ret_display.len();
        let used: usize = 3 + fn_name.len().min(name_ret_max.saturating_sub(ret_type.len() + 4))
            + if !ret_type.is_empty() { 3 + ret_display_len } else { 0 };
        let pad = w.saturating_sub(used + badges.len() + 1);
        row1.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
        // caller count — green peripheral
        if !caller_badge.is_empty() {
            row1.push(Span::styled(
                caller_badge.clone(),
                Style::default().fg(tn::CALLER_COLOR).bg(bg),
            ));
        }
        // callee count — orange peripheral
        if !callee_badge.is_empty() {
            row1.push(Span::styled(
                callee_badge.clone(),
                Style::default().fg(tn::CALLEE_COLOR).bg(bg),
            ));
        }
        // Line number — very muted
        row1.push(Span::styled(
            line_badge,
            Style::default().fg(tn::FG_DARK).bg(bg),
        ));

        all_lines.push(Line::from(row1));
        line_fn_idx.push(vi);

        // ── ROW 2: [  pub fn (params)   "summary"] ────────────────────────
        let kw_part = if !vis_kw.is_empty() {
            format!("{} ", vis_kw)
        } else {
            String::new()
        };
        let params_part = if !params.is_empty() {
            format!("({})", truncate(&params, 30))
        } else {
            String::new()
        };
        let sig_part = format!("   {}{}{}", kw_part, "fn ", params_part);
        let summary_part = if !func.summary.is_empty() {
            let max_s = w.saturating_sub(sig_part.chars().count() + 4);
            format!("  \"{}\"", truncate(&func.summary, max_s))
        } else {
            String::new()
        };

        let row2 = Line::from(vec![
            // TERTIARY: keywords and params — muted, processed peripherally
            Span::styled(
                sig_part,
                Style::default().fg(tn::FG_DARK).bg(bg),
            ),
            // SUPPORT: summary — italic, comment-colored
            Span::styled(
                summary_part,
                Style::default()
                    .fg(tn::COMMENT)
                    .bg(bg)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]);
        all_lines.push(row2);
        line_fn_idx.push(vi);

        // ── ROW 3 (selected only): call graph ─────────────────────────────
        if is_sel && (!func.callees.is_empty() || !func.callers.is_empty()) {
            let mut row3 = vec![Span::styled("   ", Style::default().bg(bg))];
            if !func.callees.is_empty() {
                row3.push(Span::styled("→ ", Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
                row3.push(Span::styled(
                    truncate(&func.callees.join("  "), w / 2 - 4),
                    Style::default().fg(tn::FG_DIM).bg(bg),
                ));
                row3.push(Span::styled("   ", Style::default().bg(bg)));
            }
            if !func.callers.is_empty() {
                row3.push(Span::styled("← ", Style::default().fg(tn::CALLER_COLOR).bg(bg)));
                row3.push(Span::styled(
                    truncate(&func.callers.join("  "), w / 2 - 4),
                    Style::default().fg(tn::FG_DIM).bg(bg),
                ));
            }
            all_lines.push(Line::from(row3));
            line_fn_idx.push(vi);
        }

        // ── Divider ────────────────────────────────────────────────────────
        if vi + 1 < visible.len() {
            all_lines.push(Line::from(Span::styled(
                " ".repeat(w),
                Style::default().bg(tn::BG),
            )));
            line_fn_idx.push(vi);
        }
    }

    // Scroll to keep selected fn in view
    let sel_start = line_fn_idx.iter().position(|&i| i == state.fn_selected).unwrap_or(0);
    let scroll = if sel_start < h / 3 {
        0
    } else {
        sel_start.saturating_sub(h / 3)
    };

    let display: Vec<Line> = all_lines.into_iter().skip(scroll).take(h).collect();
    let p = Paragraph::new(display).style(Style::default().bg(tn::BG));
    frame.render_widget(p, inner);
}

// ─── Signature decomposition ──────────────────────────────────────────────────
// Returns (visibility_keywords, fn_name, params, return_type)

fn decompose_sig(sig: &str) -> (String, String, String, String) {
    // Strip leading visibility/async keywords
    let vis_words = ["pub", "async", "unsafe", "extern", "const", "pub(crate)", "pub(super)"];
    let mut rest = sig.trim();
    let mut vis_parts: Vec<&str> = Vec::new();

    loop {
        let mut found = false;
        for &kw in &vis_words {
            if rest.starts_with(kw) {
                let after = &rest[kw.len()..];
                if after.is_empty() || after.starts_with(|c: char| !c.is_alphanumeric() && c != '_') {
                    vis_parts.push(kw);
                    rest = after.trim_start();
                    found = true;
                    break;
                }
            }
        }
        if !found { break; }
    }

    // Strip `fn ` or `def ` or `function `
    let lang_kws = ["fn ", "def ", "function ", "func "];
    for kw in &lang_kws {
        if rest.starts_with(kw) {
            rest = &rest[kw.len()..];
            break;
        }
    }

    // Extract fn name (up to `(` or `<`)
    let name_end = rest.find(|c: char| c == '(' || c == '<' || c == ':').unwrap_or(rest.len());
    let fn_name = rest[..name_end].trim().to_string();
    rest = &rest[name_end..];

    // Skip generics <...>
    if rest.starts_with('<') {
        let mut depth = 0usize;
        let mut i = 0;
        for ch in rest.chars() {
            i += ch.len_utf8();
            match ch { '<' => depth += 1, '>' => { depth -= 1; if depth == 0 { break; } } _ => {} }
        }
        rest = &rest[i..];
    }

    // Extract params (...)
    let params = if rest.starts_with('(') {
        let close = find_matching_paren(rest);
        let inner = &rest[1..close.saturating_sub(1)];
        rest = &rest[close..];
        // Simplify params: strip types, keep names only for display
        inner.trim().to_string()
    } else {
        String::new()
    };

    // Return type: after `->` or `:` (Python)
    let ret = if let Some(pos) = rest.find("->") {
        rest[pos + 2..].trim().trim_end_matches('{').trim().to_string()
    } else if let Some(pos) = rest.find(':') {
        // Python return annotation
        rest[pos + 1..].trim().to_string()
    } else {
        String::new()
    };

    (vis_parts.join(" "), fn_name, params, ret)
}

fn find_matching_paren(s: &str) -> usize {
    let mut depth = 0usize;
    let mut i = 0;
    for ch in s.chars() {
        i += ch.len_utf8();
        match ch { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { return i; } } _ => {} }
    }
    i
}

fn truncate(s: &str, max: usize) -> String {
    if max < 2 { return String::new(); }
    if s.chars().count() <= max { s.to_string() }
    else { format!("{}…", s.chars().take(max - 1).collect::<String>()) }
}
