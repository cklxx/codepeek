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

#[derive(ClapParser, Debug)]
#[command(name = "codepeek", about = "Minimal code reader — show only what matters")]
#[command(version = "0.3.0")]
struct Cli {
    /// Source file or directory
    path: PathBuf,

    /// Dump parsed functions as JSON and exit (agent/pipe mode)
    #[arg(long, short = 'j')]
    json: bool,

    /// Filter by function name (use with --json)
    #[arg(long, short = 'f')]
    function: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = resolve_path(&cli.path)?;
    let root_dir = if cli.path.is_dir() {
        cli.path.clone()
    } else {
        file_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| file_path.clone())
    };

    let mut functions = parser::parse_file(&file_path).unwrap_or_default();
    let source = std::fs::read_to_string(&file_path)?;
    analyzer::enrich_with_calls(&mut functions, &source);

    if cli.json {
        return dump_json(&functions, cli.function.as_deref());
    }

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
        eprintln!("codepeek: {}", e);
    }
    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if !event::poll(Duration::from_millis(150))? {
            continue;
        }
        let Event::Key(key) = event::read()? else { continue };

        // Global quit
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'))
        {
            return Ok(());
        }

        // Clear transient status on any keypress
        if !state.status_msg.is_empty() {
            state.status_msg.clear();
        }

        match &state.mode {
            AppMode::Search => handle_search(state, key.code),
            AppMode::Normal => {
                if handle_normal(state, key.code)? {
                    return Ok(());
                }
            }
        }
    }
}

fn handle_normal(state: &mut AppState, key: KeyCode) -> Result<bool> {
    match key {
        KeyCode::Char('q') => return Ok(true),

        // Esc: context-sensitive back
        KeyCode::Esc => match state.focus {
            PanelFocus::SourceView   => state.focus = if state.functions.is_empty() {
                PanelFocus::FileTree
            } else {
                PanelFocus::FunctionList
            },
            PanelFocus::FunctionList => state.focus = PanelFocus::FileTree,
            PanelFocus::FileTree     => return Ok(true),
        },

        // Tab: cycle forward tree → fns → source
        KeyCode::Tab => {
            state.focus = match state.focus {
                PanelFocus::FileTree => {
                    if state.functions.is_empty() { PanelFocus::SourceView }
                    else { PanelFocus::FunctionList }
                }
                PanelFocus::FunctionList => PanelFocus::SourceView,
                PanelFocus::SourceView => PanelFocus::FileTree,
            };
        }

        // BackTab / h: cycle backward / go left
        KeyCode::BackTab | KeyCode::Char('h') => {
            state.focus = match state.focus {
                PanelFocus::FileTree => PanelFocus::SourceView,
                PanelFocus::FunctionList => PanelFocus::FileTree,
                PanelFocus::SourceView => {
                    if state.functions.is_empty() { PanelFocus::FileTree }
                    else { PanelFocus::FunctionList }
                }
            };
        }

        // j / ↓  — context-sensitive
        KeyCode::Char('j') | KeyCode::Down => match state.focus {
            PanelFocus::FileTree     => state.tree_select_next(),
            PanelFocus::FunctionList => state.fn_select_next(),
            PanelFocus::SourceView   => state.source_scroll_down(1),
        },

        // k / ↑
        KeyCode::Char('k') | KeyCode::Up => match state.focus {
            PanelFocus::FileTree     => state.tree_select_prev(),
            PanelFocus::FunctionList => state.fn_select_prev(),
            PanelFocus::SourceView   => state.source_scroll_up(1),
        },

        // d / u — half-page scroll in source (works from any panel)
        KeyCode::Char('d') => state.source_scroll_down(20),
        KeyCode::Char('u') => state.source_scroll_up(20),

        // n / N — next/prev function, sync source
        KeyCode::Char('n') => {
            state.fn_select_next();
            state.focus = PanelFocus::FunctionList;
        }
        KeyCode::Char('N') => {
            state.fn_select_prev();
            state.focus = PanelFocus::FunctionList;
        }

        // g / G — go to top / bottom
        KeyCode::Char('g') => match state.focus {
            PanelFocus::FileTree     => state.tree_selected = 0,
            PanelFocus::FunctionList => { state.fn_selected = 0; state.sync_source_scroll(); }
            PanelFocus::SourceView   => state.source_scroll = 0,
        },
        KeyCode::Char('G') => match state.focus {
            PanelFocus::FileTree => {
                let n = state.tree_flat().len();
                if n > 0 { state.tree_selected = n - 1; }
            }
            PanelFocus::FunctionList => {
                let n = state.visible_fns().len();
                if n > 0 { state.fn_selected = n - 1; state.sync_source_scroll(); }
            }
            PanelFocus::SourceView => {
                state.source_scroll = state.source_lines.len().saturating_sub(1);
            }
        },

        // Enter / l — activate / go right
        KeyCode::Enter | KeyCode::Char('l') => match state.focus {
            PanelFocus::FileTree => {
                if let Some(path) = state.tree_activate() {
                    open_file(state, path)?;
                } else {
                    // dir expanded/collapsed, stay in tree
                }
            }
            PanelFocus::FunctionList => {
                state.sync_source_scroll();
                state.focus = PanelFocus::SourceView;
            }
            PanelFocus::SourceView => {}
        },

        // r — reload current file
        KeyCode::Char('r') => {
            let path = state.file_path.clone();
            open_file(state, path)?;
            state.status_msg = "reloaded".to_string();
        }

        // / — search functions
        KeyCode::Char('/') => {
            state.mode = AppMode::Search;
            state.search_query.clear();
            state.focus = PanelFocus::FunctionList;
        }

        _ => {}
    }
    Ok(false)
}

fn handle_search(state: &mut AppState, key: KeyCode) {
    match key {
        KeyCode::Esc | KeyCode::Enter => {
            state.mode = AppMode::Normal;
            state.fn_selected = 0;
            state.sync_source_scroll();
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

fn open_file(state: &mut AppState, path: PathBuf) -> Result<()> {
    let mut functions = parser::parse_file(&path).unwrap_or_default();
    let source = std::fs::read_to_string(&path).unwrap_or_default();
    analyzer::enrich_with_calls(&mut functions, &source);
    state.load_file(path, functions);
    Ok(())
}

fn resolve_path(path: &PathBuf) -> Result<PathBuf> {
    if path.is_file() { return Ok(path.clone()); }
    if path.is_dir() {
        for candidate in ["src/lib.rs", "src/main.rs"] {
            let p = path.join(candidate);
            if p.is_file() { return Ok(p); }
        }
        // First supported file in dir
        for entry in std::fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_file() {
                let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext, "rs" | "py" | "js" | "ts" | "tsx") {
                    return Ok(p);
                }
            }
        }
        // Any file at all
        for entry in std::fs::read_dir(path)? {
            let p = entry?.path();
            if p.is_file() { return Ok(p); }
        }
        anyhow::bail!("No files found in {:?}", path);
    }
    anyhow::bail!("Path does not exist: {:?}", path)
}

fn dump_json(functions: &[model::FunctionInfo], filter: Option<&str>) -> Result<()> {
    let fns: Vec<_> = match filter {
        Some(f) => functions.iter().filter(|fn_| fn_.name.contains(f)).collect(),
        None    => functions.iter().collect(),
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
        println!("    \"callees\": {:?}", f.callees);
        println!("  }}{}", comma);
    }
    println!("]");
    Ok(())
}
