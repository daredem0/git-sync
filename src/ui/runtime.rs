//! TUI-layer runtime functionality.

use super::input::handle_key_press;
use super::model::build_audit_model;
use super::render::render_page;
use super::types::{AppState, AuditModel};
use crate::app::AppConfig;
use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use std::io;
use std::time::Duration;

/// Runs the interactive TUI audit workflow for the provided config.
///
/// # Errors
///
/// Returns an error when terminal setup, rendering, or event handling fails.
pub fn run(config: &AppConfig) -> Result<()> {
    let model = build_audit_model(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app_state = AppState::new(&model);

    let loop_result = run_loop(&mut terminal, &model, &mut app_state);

    // Always attempt terminal cleanup, even when the event loop returns an error.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result
}

/// Runs the terminal event/render loop until user exit.
///
/// # Errors
///
/// Returns an error when drawing or reading events fails.
fn run_loop(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    model: &AuditModel,
    state: &mut AppState,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render_page(frame, model, state))?;

        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            if handle_key_press(state, model, key.code) {
                break;
            }
        }
    }

    Ok(())
}
