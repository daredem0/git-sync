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

trait RuntimeOps {
    type B: Backend;

    fn setup_terminal(&self) -> Result<ratatui::Terminal<Self::B>>;
    fn cleanup_terminal(&self, terminal: &mut ratatui::Terminal<Self::B>);
}

struct CrosstermEventSource {
    poll_fn: fn(Duration) -> io::Result<bool>,
    read_fn: fn() -> io::Result<Event>,
}

impl Default for CrosstermEventSource {
    fn default() -> Self {
        Self {
            poll_fn: event::poll,
            read_fn: event::read,
        }
    }
}

impl EventSource for CrosstermEventSource {
    fn poll(&self, timeout: Duration) -> io::Result<bool> {
        (self.poll_fn)(timeout)
    }

    fn read(&self) -> io::Result<Event> {
        (self.read_fn)()
    }
}

struct CrosstermRuntimeOps;

impl RuntimeOps for CrosstermRuntimeOps {
    type B = CrosstermBackend<io::Stdout>;

    fn setup_terminal(&self) -> Result<ratatui::Terminal<Self::B>> {
        setup_crossterm_terminal_with(enable_raw_mode, enter_alternate_screen)
    }

    fn cleanup_terminal(&self, terminal: &mut ratatui::Terminal<Self::B>) {
        cleanup_crossterm_terminal_with(
            terminal,
            disable_raw_mode,
            leave_alternate_screen,
            show_terminal_cursor,
        );
    }
}

fn enter_alternate_screen<W: io::Write>(writer: &mut W) -> io::Result<()> {
    execute!(writer, EnterAlternateScreen).map(|_| ())
}

fn leave_alternate_screen<W: io::Write>(backend: &mut CrosstermBackend<W>) -> io::Result<()> {
    execute!(backend, LeaveAlternateScreen).map(|_| ())
}

fn show_terminal_cursor<B: Backend>(terminal: &mut ratatui::Terminal<B>) -> io::Result<()> {
    terminal.show_cursor()
}

fn build_crossterm_terminal(
    stdout: io::Stdout,
) -> Result<ratatui::Terminal<CrosstermBackend<io::Stdout>>> {
    let backend = CrosstermBackend::new(stdout);
    ratatui::Terminal::new(backend).map_err(Into::into)
}

fn setup_terminal_with<B: Backend>(
    enable_raw_mode_fn: fn() -> io::Result<()>,
    enter_alt_screen_fn: fn(&mut io::Stdout) -> io::Result<()>,
    build_terminal_fn: fn(io::Stdout) -> Result<ratatui::Terminal<B>>,
) -> Result<ratatui::Terminal<B>> {
    enable_raw_mode_fn()?;
    let mut stdout = io::stdout();
    enter_alt_screen_fn(&mut stdout)?;
    build_terminal_fn(stdout)
}

fn setup_crossterm_terminal_with(
    enable_raw_mode_fn: fn() -> io::Result<()>,
    enter_alt_screen_fn: fn(&mut io::Stdout) -> io::Result<()>,
) -> Result<ratatui::Terminal<CrosstermBackend<io::Stdout>>> {
    setup_terminal_with(
        enable_raw_mode_fn,
        enter_alt_screen_fn,
        build_crossterm_terminal,
    )
}

fn cleanup_terminal_with<B: Backend>(
    terminal: &mut ratatui::Terminal<B>,
    disable_raw_mode_fn: fn() -> io::Result<()>,
    leave_alt_screen_fn: fn(&mut B) -> io::Result<()>,
    show_cursor_fn: fn(&mut ratatui::Terminal<B>) -> io::Result<()>,
) {
    // Always attempt terminal cleanup, even when the event loop returns an error.
    let _ = disable_raw_mode_fn();
    let _ = leave_alt_screen_fn(terminal.backend_mut());
    let _ = show_cursor_fn(terminal);
}

fn cleanup_crossterm_terminal_with(
    terminal: &mut ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    disable_raw_mode_fn: fn() -> io::Result<()>,
    leave_alt_screen_fn: fn(&mut CrosstermBackend<io::Stdout>) -> io::Result<()>,
    show_cursor_fn: fn(&mut ratatui::Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()>,
) {
    cleanup_terminal_with(
        terminal,
        disable_raw_mode_fn,
        leave_alt_screen_fn,
        show_cursor_fn,
    );
}

/// Runs the interactive TUI audit workflow for the provided config.
///
/// # Errors
///
/// Returns an error when terminal setup, rendering, or event handling fails.
pub fn run(config: &AppConfig) -> Result<()> {
    let runtime = CrosstermRuntimeOps;
    let event_source = CrosstermEventSource::default();
    #[cfg(test)]
    if let Some(result) = run_override(config) {
        return result;
    }
    run_with_runtime(config, &runtime, &event_source)
}

#[cfg(test)]
thread_local! {
    static TEST_RUN_OVERRIDE: std::cell::RefCell<Option<fn(&AppConfig) -> Result<()>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn run_override(config: &AppConfig) -> Option<Result<()>> {
    TEST_RUN_OVERRIDE.with(|slot| slot.borrow().as_ref().map(|run_fn| run_fn(config)))
}

#[cfg(test)]
pub(super) fn set_test_run_override(run_fn: Option<fn(&AppConfig) -> Result<()>>) {
    TEST_RUN_OVERRIDE.with(|slot| {
        *slot.borrow_mut() = run_fn;
    });
}

/// Runs the full terminal lifecycle with injectable setup/cleanup hooks.
fn run_with_runtime<R: RuntimeOps>(
    config: &AppConfig,
    runtime: &R,
    event_source: &impl EventSource,
) -> Result<()> {
    let mut terminal = runtime.setup_terminal()?;
    let loop_result = run_with_prepared_terminal(&mut terminal, config, event_source);
    runtime.cleanup_terminal(&mut terminal);
    loop_result
}

/// Runs loading/render logic after terminal setup has already completed.
fn run_with_prepared_terminal(
    terminal: &mut ratatui::Terminal<impl Backend>,
    config: &AppConfig,
    event_source: &impl EventSource,
) -> Result<()> {
    terminal.draw(render_loading_screen)?;
    let model = build_audit_model(config);
    run_with_loaded_model(terminal, model, event_source)
}

/// Runs the key/render loop for an already-built model.
fn run_with_loaded_model(
    terminal: &mut ratatui::Terminal<impl Backend>,
    model: AuditModel,
    event_source: &impl EventSource,
) -> Result<()> {
    let mut app_state = AppState::new(&model);
    run_loop(terminal, &model, &mut app_state, event_source)
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
