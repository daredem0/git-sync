// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Terminal runtime loop for the interactive audit UI.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

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
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::io;
use std::time::Duration;

trait EventSource {
    fn poll(&self, timeout: Duration) -> io::Result<bool>;
    fn read(&self) -> io::Result<Event>;
}

struct CrosstermEventSource;

impl EventSource for CrosstermEventSource {
    fn poll(&self, timeout: Duration) -> io::Result<bool> {
        event::poll(timeout)
    }

    fn read(&self) -> io::Result<Event> {
        event::read()
    }
}

/// Runs the interactive TUI audit workflow for the provided config.
///
/// # Errors
///
/// Returns an error when terminal setup, rendering, or event handling fails.
pub fn run(config: &AppConfig) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    terminal.draw(render_loading_screen)?;

    let model = build_audit_model(config);
    let mut app_state = AppState::new(&model);
    let event_source = CrosstermEventSource;

    let loop_result = run_loop(&mut terminal, &model, &mut app_state, &event_source);

    // Always attempt terminal cleanup, even when the event loop returns an error.
    let _ = disable_raw_mode();
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();

    loop_result
}

fn render_loading_screen(frame: &mut ratatui::Frame<'_>) {
    let area = frame.area();
    let block = Block::default().borders(Borders::ALL).title("git-sync");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Min(6),
            Constraint::Percentage(40),
        ])
        .split(inner);

    let text = "Loading audit view...\n\nThis can take a while on large bundles.\n\nTip: `git-sync audit --format table` is a fast non-interactive proof run.";
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });
    frame.render_widget(paragraph, chunks[1]);
}

/// Runs the terminal event/render loop until user exit.
///
/// # Errors
///
/// Returns an error when drawing or reading events fails.
fn run_loop(
    terminal: &mut ratatui::Terminal<impl Backend>,
    model: &AuditModel,
    state: &mut AppState,
    event_source: &impl EventSource,
) -> Result<()> {
    loop {
        terminal.draw(|frame| render_page(frame, model, state))?;

        if event_source.poll(Duration::from_millis(200))?
            && let Event::Key(key) = event_source.read()?
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

#[cfg(test)]
mod tests;
