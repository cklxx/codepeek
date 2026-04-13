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

use model::{AppMode, AppState};

#[derive(ClapParser, Debug)]
#[command(name = "codepeek", about = "Minimal code reader for humans")]
#[command(version = "0.1.0")]
struct Cli {
    /// Path to a source file or directory
    path: PathBuf,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let file_path = resolve_path(&cli.path)?;

    let mut functions = parser::parse_file(&file_path)
        .map_err(|e| anyhow::anyhow!("Parse error: {}", e))?;

    if functions.is_empty() {
        eprintln!("No functions found in {:?}", file_path);
        return Ok(());
    }

    let source = std::fs::read_to_string(&file_path)?;
    analyzer::enrich_with_calls(&mut functions, &source);

    let mut state = AppState::new(file_path, functions);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut state);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = res {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::render(f, state))?;

        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match &state.mode {
                    AppMode::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => return Ok(()),
                        KeyCode::Esc => return Ok(()),
                        KeyCode::Char('j') | KeyCode::Down => state.select_next(),
                        KeyCode::Char('k') | KeyCode::Up => state.select_prev(),
                        KeyCode::Enter | KeyCode::Char(' ') => state.toggle_expand(),
                        KeyCode::Char('g') => state.selected = 0,
                        KeyCode::Char('G') => {
                            let len = state.visible_functions().len();
                            if len > 0 {
                                state.selected = len - 1;
                            }
                        }
                        KeyCode::Char('/') => {
                            state.mode = AppMode::Search;
                            state.search_query.clear();
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    AppMode::Search => match key.code {
                        KeyCode::Esc | KeyCode::Enter => {
                            state.mode = AppMode::Normal;
                            state.selected = 0;
                        }
                        KeyCode::Char(c) => {
                            state.search_query.push(c);
                            state.selected = 0;
                        }
                        KeyCode::Backspace => {
                            state.search_query.pop();
                            state.selected = 0;
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}

fn resolve_path(path: &PathBuf) -> Result<PathBuf> {
    if path.is_file() {
        return Ok(path.clone());
    }

    if path.is_dir() {
        let extensions = ["rs", "py", "js", "ts", "tsx"];
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_file() {
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext) {
                        return Ok(p);
                    }
                }
            }
        }
        anyhow::bail!("No supported source files found in directory {:?}", path);
    }

    anyhow::bail!("Path does not exist: {:?}", path)
}
