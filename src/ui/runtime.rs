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
use ratatui::backend::{Backend, CrosstermBackend};
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
    let model = build_audit_model(config);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;
    let mut app_state = AppState::new(&model);
    let event_source = CrosstermEventSource;

    let loop_result = run_loop(&mut terminal, &model, &mut app_state, &event_source);

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
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::backend::TestBackend;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::io::ErrorKind;

    struct ScriptedEventSource {
        polls: RefCell<VecDeque<io::Result<bool>>>,
        reads: RefCell<VecDeque<io::Result<Event>>>,
    }

    impl ScriptedEventSource {
        fn new(polls: Vec<io::Result<bool>>, reads: Vec<io::Result<Event>>) -> Self {
            Self {
                polls: RefCell::new(polls.into()),
                reads: RefCell::new(reads.into()),
            }
        }
    }

    impl EventSource for ScriptedEventSource {
        fn poll(&self, _timeout: Duration) -> io::Result<bool> {
            self.polls.borrow_mut().pop_front().unwrap_or(Ok(false))
        }

        fn read(&self) -> io::Result<Event> {
            self.reads.borrow_mut().pop_front().unwrap_or_else(|| {
                Err(io::Error::new(
                    ErrorKind::UnexpectedEof,
                    "no scripted event available",
                ))
            })
        }
    }

    #[test]
    fn run_loop_exits_when_quit_key_is_pressed() {
        let model = super::super::tests::support::sample_model(1, 1);
        let mut state = AppState::new(&model);
        let mut terminal =
            ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
        let events = ScriptedEventSource::new(
            vec![Ok(true)],
            vec![Ok(Event::Key(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            )))],
        );

        let result = run_loop(&mut terminal, &model, &mut state, &events);
        assert!(
            result.is_ok(),
            "run loop should stop cleanly when quit key is routed"
        );
    }

    #[test]
    fn run_loop_ignores_non_press_key_events() {
        let model = super::super::tests::support::sample_model(1, 1);
        let mut state = AppState::new(&model);
        let mut terminal =
            ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
        let events = ScriptedEventSource::new(
            vec![Ok(true), Ok(true)],
            vec![
                Ok(Event::Key(KeyEvent {
                    code: KeyCode::Char('?'),
                    modifiers: KeyModifiers::NONE,
                    kind: KeyEventKind::Release,
                    state: KeyEventState::NONE,
                })),
                Ok(Event::Key(KeyEvent::new(
                    KeyCode::Char('q'),
                    KeyModifiers::NONE,
                ))),
            ],
        );

        let result = run_loop(&mut terminal, &model, &mut state, &events);
        assert!(
            result.is_ok(),
            "run loop should continue after ignored non-press key events"
        );
        assert!(
            !state.show_help,
            "release key events must not toggle help state"
        );
    }

    #[test]
    fn run_loop_propagates_event_read_errors() {
        let model = super::super::tests::support::sample_model(1, 1);
        let mut state = AppState::new(&model);
        let mut terminal =
            ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
        let events = ScriptedEventSource::new(
            vec![Ok(true)],
            vec![Err(io::Error::new(
                ErrorKind::ConnectionAborted,
                "scripted read failure",
            ))],
        );

        let error = run_loop(&mut terminal, &model, &mut state, &events)
            .expect_err("event read errors should stop the run loop");
        assert!(
            error.to_string().contains("scripted read failure"),
            "error text should preserve read failure details"
        );
    }
}
