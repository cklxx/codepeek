/// Function list — Visual hierarchy from eye-tracking + attention research:
///
/// ─── impl OwnerType ───────────────────────────────   <- GROUP HEADER: amber anchor
///  · name              → RetType            L123 ↑↓   <- ROW 1: name (blue) + type (cyan)
///    async (&self, arg…)   "docstring summary"         <- ROW 2: sig muted + summary italic
/// ─────────────────────────────────────────────────
///  ▶ selected_name     → RetType            L89 ↑1    <- SELECTED: amber name
///    (&mut self, x: T)   "Updates the state"
///    → callee1  callee2      ← caller1                 <- CALL GRAPH (selected only)
///
/// Attention levels (WCAG/APCA contrast vs #1a1b26):
///   OWNER    #e0af68  8.9:1  warm amber   ← structural anchor, preattentive (>180° from blue)
///   NAME     #7aa2f7  6.8:1  cool blue    ← primary identifier, first fixation
///   TYPE_    #2ac3de  8.1:1  cyan         ← secondary, return type
///   ASYNC    #9d7cd8  5.8:1  violet       ← modifier badge
///   KW_DEF   #7982aa  4.8:1  muted blue   ← signature keywords (fn/pub/self)
///   COMMENT  #565f89  2.8:1  dark grey    ← summary (intentionally below AA, skim tier)
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::model::{AppState, FunctionInfo, PanelFocus};
use super::highlight::tn;

pub fn render_function_list(frame: &mut Frame, state: &AppState, area: Rect) {
    let focused = state.focus == PanelFocus::FunctionList;

    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" fns ", Style::default().fg(tn::FG_DIM)),
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
                " no functions — source view →"
            } else {
                " no match"
            })
            .style(Style::default().fg(tn::FG_DARK).bg(tn::BG)),
            inner,
        );
        return;
    }

    let w = inner.width as usize;
    let h = inner.height as usize;

    // Does any function have an owner? Determines whether to show group headers.
    let has_owners = visible.iter().any(|f| f.owner.is_some());

    let mut all_lines: Vec<Line> = Vec::new();
    // Maps each rendered line index → function index in `visible`
    let mut line_fn_idx: Vec<usize> = Vec::new();

    let mut last_owner: Option<Option<String>> = None; // None = no group rendered yet

    for (vi, func) in visible.iter().enumerate() {
        let cur_owner = Some(func.owner.clone());

        // ── Group header on owner transition ──────────────────────────────────
        if has_owners && cur_owner != last_owner {
            // Blank gap before second+ groups (not before first)
            if last_owner.is_some() {
                all_lines.push(Line::from(Span::raw("")));
                line_fn_idx.push(vi);
            }
            all_lines.push(group_header_line(func.owner.as_deref(), w));
            line_fn_idx.push(vi);
            last_owner = cur_owner;
        }

        let is_sel = vi == state.fn_selected;
        let bg = if is_sel { tn::BG_SEL } else { tn::BG };

        let (vis_kw, fn_name, params, ret) = decompose_sig(&func.signature);

        // ── ROW 1: [▶/·] name → RetType              badges ──────────────────
        let arrow = if is_sel { "▶ " } else { "  " };
        let name_fg = if is_sel { tn::SELECTED_FG } else { tn::NAME };

        // Allocate width: name gets up to 45%, ret up to 22%, rest for badges
        let name_max = (w * 45 / 100).max(8);
        let ret_max  = (w * 22 / 100).max(4);

        let badge_str = build_badge(func);
        let badge_w = badge_str.chars().count();

        // Truncate name and ret to fit
        let name_trunc = take_w(&fn_name, name_max);
        let ret_trunc  = if ret.is_empty() { String::new() } else {
            take_w(&format!("→ {}", ret), ret_max)
        };

        // Padding between ret and badges
        let used = 2 + name_trunc.chars().count()
            + if ret_trunc.is_empty() { 0 } else { ret_trunc.chars().count() + 1 };
        let pad = w.saturating_sub(used + badge_w).min(w);

        let mut row1 = vec![
            Span::styled(
                arrow,
                Style::default().fg(if is_sel { tn::OWNER } else { tn::FG_DARK }).bg(bg),
            ),
        ];

        // Name: with search-match highlighting when search is active
        append_name_spans(&mut row1, &name_trunc, name_fg, &state.search_query, is_sel, bg);

        if !ret_trunc.is_empty() {
            row1.push(Span::styled(" ", Style::default().bg(bg)));
            row1.push(Span::styled(
                ret_trunc,
                Style::default().fg(tn::TYPE_).bg(bg),
            ));
        }
        row1.push(Span::styled(" ".repeat(pad), Style::default().bg(bg)));
        row1.push(Span::styled(badge_str, Style::default().fg(tn::FG_DARK).bg(bg)));

        all_lines.push(Line::from(row1));
        line_fn_idx.push(vi);

        // ── ROW 2: modifiers (params)   "summary" ────────────────────────────
        all_lines.push(row2_line(func, &vis_kw, &params, bg, w));
        line_fn_idx.push(vi);

        // ── ROW 3 (selected only): call graph ────────────────────────────────
        if is_sel && (!func.callees.is_empty() || !func.callers.is_empty()) {
            all_lines.push(callgraph_line(func, bg, w));
            line_fn_idx.push(vi);
        }

        // ── Thin divider between functions ───────────────────────────────────
        if vi + 1 < visible.len() {
            all_lines.push(divider_line(w));
            line_fn_idx.push(vi);
        }
    }

    // Scroll to keep selected function in view
    let sel_first_line = line_fn_idx
        .iter()
        .position(|&i| i == state.fn_selected)
        .unwrap_or(0);
    let scroll = sel_first_line.saturating_sub(h / 4);

    let display: Vec<Line> = all_lines.into_iter().skip(scroll).take(h).collect();
    frame.render_widget(
        Paragraph::new(display).style(Style::default().bg(tn::BG)),
        inner,
    );
}

// ─── Line builders ───────────────────────────────────────────────────────────

fn group_header_line<'a>(owner: Option<&str>, w: usize) -> Line<'a> {
    match owner {
        Some(name) => {
            // Format: ─ impl Name ─────────────────
            let prefix = " impl ";
            let suffix_len = w.saturating_sub(1 + prefix.len() + name.len() + 1);
            let suffix = "─".repeat(suffix_len);
            Line::from(vec![
                Span::styled("─", Style::default().fg(tn::BORDER_IDLE)),
                Span::styled(prefix, Style::default().fg(tn::FG_DARK)),
                Span::styled(
                    name.to_string(),
                    Style::default().fg(tn::OWNER).add_modifier(Modifier::BOLD),
                ),
                Span::styled(" ", Style::default().fg(tn::BORDER_IDLE)),
                Span::styled(suffix, Style::default().fg(tn::BORDER_IDLE)),
            ])
        }
        None => {
            // Free functions section
            let label = "─ fn ";
            let fill = "─".repeat(w.saturating_sub(label.len()));
            Line::from(vec![
                Span::styled(label.to_string(), Style::default().fg(tn::FG_DARK)),
                Span::styled(fill, Style::default().fg(tn::BORDER_IDLE)),
            ])
        }
    }
}

fn row2_line<'a>(
    func: &FunctionInfo,
    vis_kw: &str,
    params: &str,
    bg: ratatui::style::Color,
    w: usize,
) -> Line<'a> {
    // Build: "  [async] (params…)   "summary""
    let mut spans = vec![Span::styled("  ", Style::default().bg(bg))];

    // async badge: violet, distinct from both warm and cool primary tokens
    if func.is_async {
        spans.push(Span::styled(
            "⚡ ",
            Style::default().fg(tn::ASYNC_TAG).bg(bg),
        ));
    }

    // Visibility prefix (pub/pub(crate) etc.) — very muted, peripheral
    if !vis_kw.is_empty() {
        let filtered = vis_kw
            .split_whitespace()
            .filter(|&kw| kw != "async") // async shown via ⚡ badge
            .collect::<Vec<_>>()
            .join(" ");
        if !filtered.is_empty() {
            spans.push(Span::styled(
                format!("{} ", filtered),
                Style::default().fg(tn::KW_DEF).bg(bg),
            ));
        }
    }

    // Self receiver — shown more prominently as it tells you if it's a method
    let self_display = extract_self_display(params);
    let rest_params  = params_without_self(params);

    if !self_display.is_empty() {
        spans.push(Span::styled(
            format!("({})", self_display),
            Style::default().fg(tn::KW_DEF).bg(bg),
        ));
        if !rest_params.is_empty() {
            // Show count of additional params rather than full expansion (saves space)
            let count = rest_params.split(',').count();
            spans.push(Span::styled(
                format!(" +{}", count),
                Style::default().fg(tn::FG_DARK).bg(bg),
            ));
        }
    } else if !rest_params.is_empty() {
        // Free function: show first param hint
        let hint = take_w(&rest_params, 20);
        spans.push(Span::styled(
            format!("({})", hint),
            Style::default().fg(tn::KW_DEF).bg(bg),
        ));
    } else {
        spans.push(Span::styled("()", Style::default().fg(tn::FG_DARK).bg(bg)));
    }

    // Summary — italic, intentionally at muted (2.8:1) contrast, skim tier
    if !func.summary.is_empty() {
        let so_far: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let avail = w.saturating_sub(so_far + 4);
        if avail > 8 {
            let summary = take_w(&func.summary, avail);
            spans.push(Span::styled("  ", Style::default().bg(bg)));
            spans.push(Span::styled(
                format!("\"{}\"", summary),
                Style::default()
                    .fg(tn::COMMENT)
                    .bg(bg)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
    }

    Line::from(spans)
}

fn callgraph_line<'a>(func: &FunctionInfo, bg: ratatui::style::Color, w: usize) -> Line<'a> {
    let half = w.saturating_sub(4) / 2;
    let mut spans = vec![Span::styled("  ", Style::default().bg(bg))];

    if !func.callees.is_empty() {
        spans.push(Span::styled("→ ", Style::default().fg(tn::CALLEE_COLOR).bg(bg)));
        spans.push(Span::styled(
            take_w(&func.callees.join("  "), half),
            Style::default().fg(tn::FG_MED).bg(bg),
        ));
    }
    if !func.callers.is_empty() {
        spans.push(Span::styled("  ← ", Style::default().fg(tn::CALLER_COLOR).bg(bg)));
        spans.push(Span::styled(
            take_w(&func.callers.join("  "), half),
            Style::default().fg(tn::FG_MED).bg(bg),
        ));
    }

    Line::from(spans)
}

fn divider_line<'a>(w: usize) -> Line<'a> {
    Line::from(Span::styled(
        format!("  {}", "·".repeat(w.saturating_sub(3))),
        Style::default().fg(tn::BORDER_IDLE),
    ))
}

fn build_badge(func: &FunctionInfo) -> String {
    let mut b = String::new();
    if !func.callers.is_empty() { b.push_str(&format!(" ↑{}", func.callers.len())); }
    if !func.callees.is_empty() { b.push_str(&format!(" ↓{}", func.callees.len())); }
    b.push_str(&format!(" L{}", func.line_range.0));
    b
}

/// Append name spans — splits into before/match/after when search is active.
fn append_name_spans(
    spans: &mut Vec<Span<'static>>,
    name: &str,
    fg: ratatui::style::Color,
    query: &str,
    is_sel: bool,
    bg: ratatui::style::Color,
) {
    let base_style = Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD);

    if !query.is_empty() && !is_sel {
        let q_lower = query.to_lowercase();
        let n_lower = name.to_lowercase();
        if let Some(pos) = n_lower.find(q_lower.as_str()) {
            let q_len = q_lower.len();
            spans.push(Span::styled(name[..pos].to_string(), base_style));
            spans.push(Span::styled(
                name[pos..pos + q_len].to_string(),
                Style::default()
                    .fg(tn::STRING)
                    .bg(bg)
                    .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            ));
            spans.push(Span::styled(name[pos + q_len..].to_string(), base_style));
            return;
        }
    }
    spans.push(Span::styled(name.to_string(), base_style));
}

// ─── Signature decomposition ─────────────────────────────────────────────────

/// Returns (visibility_keywords, fn_name, params_inner, return_type)
fn decompose_sig(sig: &str) -> (String, String, String, String) {
    let vis_kws  = ["pub(crate)", "pub(super)", "pub", "async", "unsafe", "extern", "const"];
    let lang_kws = ["fn ", "def ", "function ", "func "];

    let mut rest = sig.trim();
    let mut vis  = Vec::new();

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

    for kw in &lang_kws {
        if rest.starts_with(kw) {
            rest = &rest[kw.len()..];
            break;
        }
    }

    let name_end = rest.find(|c: char| c == '(' || c == '<' || c == ':').unwrap_or(rest.len());
    let fn_name  = rest[..name_end].trim().to_string();
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

    let params = if rest.starts_with('(') {
        let close = find_close_paren(rest);
        let inner = rest[1..close.saturating_sub(1)].trim().to_string();
        rest = &rest[close..];
        inner
    } else {
        String::new()
    };

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

/// Extract the self-receiver display from a params string.
fn extract_self_display(params: &str) -> &str {
    let s = params.trim();
    if s.starts_with("&mut self") { "&mut self" }
    else if s.starts_with("&self")  { "&self" }
    else if s.starts_with("mut self") { "mut self" }
    else if s.starts_with("self")   { "self" }
    else { "" }
}

/// Params string with the self receiver removed.
fn params_without_self(params: &str) -> String {
    let s = params.trim();
    for prefix in &["&mut self, ", "&self, ", "mut self, ", "self, ", "&mut self", "&self", "mut self", "self"] {
        if s.starts_with(prefix) {
            return s[prefix.len()..].trim().to_string();
        }
    }
    s.to_string()
}

fn take_w(s: &str, max: usize) -> String {
    if max == 0 { return String::new(); }
    if s.chars().count() <= max { return s.to_string(); }
    format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
}
