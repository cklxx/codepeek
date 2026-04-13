/// Function list — visual hierarchy from eye-tracking research:
///
///   Row 1  [·/▶] fn_name  → RetType            ↑callers L123
///   Row 2        pub fn (params)  "summary italic"
///   Row 3  (selected only) → callees  ← callers
///
/// Attention levels:
///   PRIMARY   fn_name: bold bright blue — first fixation point
///   SECONDARY RetType: cyan — second glance
///   TERTIARY  pub fn params: very muted — peripheral
///   SUPPORT   summary: italic dim — read only if needed
///   PERIPHERAL badges ↑↓ line-num: barely visible right side
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, PanelFocus};
use super::highlight::tn;

pub fn render_function_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FunctionList;

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(
                format!(" {} fns ", state.functions.len()),
                Style::default().fg(tn::FG_DIM),
            ),
        ]))
        .borders(Borders::ALL)
        .border_type(if focused { BorderType::Rounded } else { BorderType::Plain })
        .border_style(Style::default().fg(if focused { tn::BORDER_FOCUS } else { tn::BORDER_IDLE }));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let visible = state.visible_fns();
    if visible.is_empty() {
        frame.render_widget(
            Paragraph::new(if state.search_query.is_empty() {
                " no functions — see source →".to_string()
            } else {
                format!(" no match '{}'", state.search_query)
            })
            .style(Style::default().fg(tn::FG_DARK).bg(tn::BG)),
            inner,
        );
        return;
    }

    let w = inner.width as usize;
    let h = inner.height as usize;

    let mut all_lines: Vec<Line> = Vec::new();
    let mut line_fn_idx: Vec<usize> = Vec::new();

    for (vi, func) in visible.iter().enumerate() {
        let is_sel = vi == state.fn_selected;
        let bg = if is_sel { tn::BG_SEL } else { tn::BG };

        let (vis_kw, fn_name, params, ret) = decompose_sig(&func.signature);

        // ── ROW 1: [▶/·] NAME → RetType           ↑N L123 ─────────────
        let arrow = if is_sel { "▶ " } else { "  " };
        let name_w = (w / 2).min(fn_name.chars().count() + 2);
        let ret_str = if ret.is_empty() { String::new() } else { format!("→ {}", ret) };
        let badge_str = {
            let mut b = String::new();
            if !func.callers.is_empty() { b.push_str(&format!(" ↑{}", func.callers.len())); }
            if !func.callees.is_empty() { b.push_str(&format!(" ↓{}", func.callees.len())); }
            b.push_str(&format!(" L{}", func.line_range.0));
            b
        };

        // Compute padding to push badges to the right
        let used = 2 + fn_name.chars().count().min(name_w)
            + if ret_str.is_empty() { 0 } else { 1 + ret_str.chars().count() };
        let pad = w.saturating_sub(used + badge_str.chars().count() + 1);

        let mut row1 = vec![
            Span::styled(arrow, Style::default()
                .fg(if is_sel { tn::SELECTED_FG } else { tn::FG_DARK }).bg(bg)),
            // PRIMARY: function name
            Span::styled(
                take_w(&fn_name, name_w),
                Style::default()
                    .fg(if is_sel { tn::SELECTED_FG } else { tn::NAME })
                    .bg(bg)
                    .add_modifier(Modifier::BOLD),
            ),
        ];
        if !ret_str.is_empty() {
            row1.push(Span::styled(" ", Style::default().bg(bg)));
            // SECONDARY: return type
            row1.push(Span::styled(
                take_w(&ret_str, w.saturating_sub(used + pad + badge_str.len())),
                Style::default().fg(tn::TYPE_).bg(bg),
            ));
        }
        // Filler + peripheral badges
        row1.push(Span::styled(" ".repeat(pad.min(w)), Style::default().bg(bg)));
        row1.push(Span::styled(badge_str, Style::default().fg(tn::FG_DARK).bg(bg)));

        all_lines.push(Line::from(row1));
        line_fn_idx.push(vi);

        // ── ROW 2: tertiary sig + italic summary ──────────────────────
        let kw_part = if vis_kw.is_empty() { "fn ".to_string() }
                      else { format!("{} fn ", vis_kw) };
        let param_part = if params.is_empty() { String::new() }
                         else { format!("({})", take_w(&params, 28)) };
        let sig_part = take_w(&format!("  {}{}", kw_part, param_part), w / 2);

        let summary_max = w.saturating_sub(sig_part.chars().count() + 3);
        let summary_part = if func.summary.is_empty() { String::new() }
                           else { format!("  \"{}\"", take_w(&func.summary, summary_max)) };

        all_lines.push(Line::from(vec![
            // TERTIARY: keywords and params — very muted
            Span::styled(sig_part, Style::default().fg(tn::FG_DARK).bg(bg)),
            // SUPPORT: summary — italic, barely visible
            Span::styled(
                summary_part,
                Style::default().fg(tn::FG_DIM).bg(bg).add_modifier(Modifier::ITALIC),
            ),
        ]));
        line_fn_idx.push(vi);

        // ── ROW 3 (selected): call graph ──────────────────────────────
        if is_sel && (!func.callees.is_empty() || !func.callers.is_empty()) {
            let mut r3 = vec![Span::styled("  ", Style::default().bg(bg))];
            if !func.callees.is_empty() {
                r3.push(Span::styled("→ ", Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
                r3.push(Span::styled(
                    take_w(&func.callees.join("  "), w.saturating_sub(4) / 2),
                    Style::default().fg(tn::FG_MED).bg(bg),
                ));
                r3.push(Span::styled("   ", Style::default().bg(bg)));
            }
            if !func.callers.is_empty() {
                r3.push(Span::styled("← ", Style::default().fg(tn::CALLER_COLOR).bg(bg)));
                r3.push(Span::styled(
                    take_w(&func.callers.join("  "), w.saturating_sub(4) / 2),
                    Style::default().fg(tn::FG_MED).bg(bg),
                ));
            }
            all_lines.push(Line::from(r3));
            line_fn_idx.push(vi);
        }

        // ── Thin divider ──────────────────────────────────────────────
        if vi + 1 < visible.len() {
            all_lines.push(Line::from(Span::styled(
                format!("  {}", "─".repeat(w.saturating_sub(3))),
                Style::default().fg(tn::FG_DARK).bg(tn::BG),
            )));
            line_fn_idx.push(vi);
        }
    }

    // Scroll to keep selected fn in view
    let sel_start = line_fn_idx.iter().position(|&i| i == state.fn_selected).unwrap_or(0);
    let scroll = sel_start.saturating_sub(h / 3);

    let display: Vec<Line> = all_lines.into_iter().skip(scroll).take(h).collect();
    frame.render_widget(
        Paragraph::new(display).style(Style::default().bg(tn::BG)),
        inner,
    );
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn take_w(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    if s.chars().count() <= max { return s.to_string(); }
    format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
}

/// Returns (visibility_keywords, fn_name, params_inner, return_type)
fn decompose_sig(sig: &str) -> (String, String, String, String) {
    let vis_kws = ["pub(crate)", "pub(super)", "pub", "async", "unsafe", "extern", "const"];
    let lang_kws = ["fn ", "def ", "function ", "func "];

    let mut rest = sig.trim();
    let mut vis = Vec::new();

    // Strip visibility/modifier keywords
    'outer: loop {
        for &kw in &vis_kws {
            if rest.starts_with(kw) {
                let after = &rest[kw.len()..];
                if after.is_empty() || !after.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                    vis.push(kw.trim());
                    rest = after.trim_start();
                    continue 'outer;
                }
            }
        }
        break;
    }

    // Strip language keyword
    for kw in &lang_kws {
        if rest.starts_with(kw) {
            rest = &rest[kw.len()..];
            break;
        }
    }

    // Function name (up to `(` or `<`)
    let name_end = rest.find(|c: char| c == '(' || c == '<' || c == ':').unwrap_or(rest.len());
    let fn_name = rest[..name_end].trim().to_string();
    rest = &rest[name_end..];

    // Skip generics
    if rest.starts_with('<') {
        let mut depth = 0usize;
        let mut consumed = 0usize;
        for ch in rest.chars() {
            consumed += ch.len_utf8();
            match ch { '<' => depth += 1, '>' => { depth -= 1; if depth == 0 { break; } } _ => {} }
        }
        rest = &rest[consumed..];
    }

    // Params
    let params = if rest.starts_with('(') {
        let close = find_close_paren(rest);
        let inner = rest[1..close.saturating_sub(1)].trim().to_string();
        rest = &rest[close..];
        inner
    } else {
        String::new()
    };

    // Return type after `->`
    let ret = if let Some(p) = rest.find("->") {
        rest[p + 2..].trim().trim_end_matches('{').trim().to_string()
    } else {
        String::new()
    };

    (vis.join(" "), fn_name, params, ret)
}

fn find_close_paren(s: &str) -> usize {
    let mut depth = 0usize;
    let mut i = 0usize;
    for ch in s.chars() {
        i += ch.len_utf8();
        match ch { '(' => depth += 1, ')' => { depth -= 1; if depth == 0 { return i; } } _ => {} }
    }
    i
}
