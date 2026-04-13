mod model;
mod parser;
mod analyzer;
mod ui;

use std::{io, path::PathBuf, time::Duration};

use anyhow::Result;
use clap::Parser as ClapParser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use model::{AppMode, AppState, PanelFocus};

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(ClapParser, Debug)]
#[command(name = "codepeek", about = "Minimal code reader for humans — and agents")]
#[command(version = "0.2.0")]
struct Cli {
    /// Source file or directory to open
    path: PathBuf,

    /// Dump parsed functions as JSON (for agent/piped use), then exit
    #[arg(long, short = 'j')]
    json: bool,

    /// Filter to a specific function name (used with --json)
    #[arg(long, short = 'f')]
    function: Option<String>,
}

// ─── Entry ───────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = resolve_path(&cli.path)?;
    let root_dir = if cli.path.is_dir() {
        cli.path.clone()
    } else {
        file_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| file_path.clone())
    };

    let mut functions = parser::parse_file(&file_path)
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    if functions.is_empty() {
        eprintln!("codepeek: no functions found in {:?}", file_path);
        return Ok(());
    }

    let source = std::fs::read_to_string(&file_path)?;
    analyzer::enrich_with_calls(&mut functions, &source);

    // ── Agent / JSON mode ─────────────────────────────────────────────────
    if cli.json {
        return dump_json(&functions, cli.function.as_deref());
    }

    // ── TUI mode ──────────────────────────────────────────────────────────
    let mut state = AppState::new(file_path, functions, root_dir);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("codepeek error: {}", e);
    }

    Ok(())
}

// ─── TUI loop ────────────────────────────────────────────────────────────────

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if !event::poll(Duration::from_millis(150))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        // Ctrl-C / Ctrl-Q always quit
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            return Ok(());
        }

        match &state.mode {
            AppMode::Search => handle_search_key(state, key.code),
            AppMode::Normal => {
                if handle_global_key(state, key.code)? {
                    return Ok(());
                }
            }
        }
    }
}

/// Returns true if app should quit.
fn handle_global_key(state: &mut AppState, key: KeyCode) -> Result<bool> {
    match key {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),

        // Panel focus toggle
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::FileTree     => PanelFocus::FunctionList,
                PanelFocus::FunctionList => PanelFocus::FileTree,
            };
        }

        // Navigation — routes to focused panel
        KeyCode::Char('j') | KeyCode::Down => match state.focus {
            PanelFocus::FileTree     => state.tree_select_next(),
            PanelFocus::FunctionList => state.fn_select_next(),
        },
        KeyCode::Char('k') | KeyCode::Up => match state.focus {
            PanelFocus::FileTree     => state.tree_select_prev(),
            PanelFocus::FunctionList => state.fn_select_prev(),
        },

        // Go to top / bottom
        KeyCode::Char('g') => match state.focus {
            PanelFocus::FileTree     => state.tree_selected = 0,
            PanelFocus::FunctionList => state.fn_selected = 0,
        },
        KeyCode::Char('G') => match state.focus {
            PanelFocus::FileTree => {
                let len = state.tree_flat().len();
                if len > 0 { state.tree_selected = len - 1; }
            }
            PanelFocus::FunctionList => {
                let len = state.visible_fns().len();
                if len > 0 { state.fn_selected = len - 1; }
            }
        },

        // Enter / Space — activate
        KeyCode::Enter | KeyCode::Char(' ') | KeyCode::Char('l') => match state.focus {
            PanelFocus::FileTree => {
                if let Some(path) = state.tree_activate() {
                    load_file(state, path)?;
                }
            }
            PanelFocus::FunctionList => state.fn_toggle_expand(),
        },

        // Search (function list only)
        KeyCode::Char('/') => {
            state.mode = AppMode::Search;
            state.search_query.clear();
            state.focus = PanelFocus::FunctionList;
        }

        _ => {}
    }
    Ok(false)
}

fn handle_search_key(state: &mut AppState, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Enter => {
            state.mode = AppMode::Normal;
            state.fn_selected = 0;
        }
        KeyCode::Char(c) => {
            state.search_query.push(c);
            state.fn_selected = 0;
        }
        KeyCode::Backspace => {
            state.search_query.pop();
            state.fn_selected = 0;
        }
        _ => {}
    }
}

// ─── File loading ─────────────────────────────────────────────────────────────

fn load_file(state: &mut AppState, path: PathBuf) -> Result<()> {
    let mut functions = match parser::parse_file(&path) {
        Ok(f) => f,
        Err(e) => {
            state.status_msg = format!("parse error: {}", e);
            return Ok(());
        }
    };
    if functions.is_empty() {
        state.status_msg = format!("no functions found in {}", path.display());
        return Ok(());
    }
    let source = std::fs::read_to_string(&path)?;
    analyzer::enrich_with_calls(&mut functions, &source);

    state.functions = functions;
    state.file_path = path;
    state.fn_selected = 0;
    state.fn_scroll = 0;
    state.search_query.clear();
    state.status_msg.clear();
    state.focus = PanelFocus::FunctionList;
    Ok(())
}

// ─── Agent JSON output ────────────────────────────────────────────────────────

fn dump_json(functions: &[model::FunctionInfo], filter: Option<&str>) -> Result<()> {
    let fns: Vec<&model::FunctionInfo> = if let Some(name) = filter {
        functions.iter().filter(|f| f.name.contains(name)).collect()
    } else {
        functions.iter().collect()
    };

    println!("[");
    for (i, f) in fns.iter().enumerate() {
        let comma = if i + 1 < fns.len() { "," } else { "" };
        println!("  {{");
        println!("    \"name\": {:?},", f.name);
        println!("    \"signature\": {:?},", f.signature);
        println!("    \"lines\": [{}, {}],", f.line_range.0, f.line_range.1);
        println!("    \"summary\": {:?},", f.summary);
        println!("    \"callers\": {:?},", f.callers);
        println!("    \"callees\": {:?},", f.callees);
        println!("    \"core\": {:?}", f.core_lines);
        println!("  }}{}", comma);
    }
    println!("]");
    Ok(())
}

// ─── Path resolution ──────────────────────────────────────────────────────────

fn resolve_path(path: &PathBuf) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.clone());
    }
    if path.is_dir() {
        let exts = ["rs", "py", "js", "ts", "tsx"];
        // Prefer src/lib.rs or src/main.rs
        for candidate in ["src/lib.rs", "src/main.rs"] {
            let p = path.join(candidate);
            if p.is_file() {
                return Ok(p);
            }
        }
        // Fall back to first matching file
        for entry in std::fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if exts.contains(&ext) {
                        return Ok(p);
                    }
                }
            }
        }
        anyhow::bail!("No supported source files found in {:?}", path);
    }
    anyhow::bail!("Path does not exist: {:?}", path)
}
