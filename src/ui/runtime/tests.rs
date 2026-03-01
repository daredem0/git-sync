// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/runtime.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

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

#[test]
fn run_loop_ignores_non_key_events_and_poll_false_cycles() {
    let model = super::super::tests::support::sample_model(1, 1);
    let mut state = AppState::new(&model);
    let mut terminal =
        ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
    let events = ScriptedEventSource::new(
        vec![Ok(false), Ok(true), Ok(true)],
        vec![
            Ok(Event::Resize(120, 40)),
            Ok(Event::Key(KeyEvent::new(
                KeyCode::Char('q'),
                KeyModifiers::NONE,
            ))),
        ],
    );

    let result = run_loop(&mut terminal, &model, &mut state, &events);
    assert!(
        result.is_ok(),
        "run loop should continue across poll-false and non-key events"
    );
}

#[test]
fn run_loop_propagates_scripted_read_fallback_error_when_no_event_is_available() {
    let model = super::super::tests::support::sample_model(1, 1);
    let mut state = AppState::new(&model);
    let mut terminal =
        ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
    let events = ScriptedEventSource::new(vec![Ok(true)], vec![]);

    let error = run_loop(&mut terminal, &model, &mut state, &events)
        .expect_err("missing scripted event should surface as read fallback error");
    assert!(
        error.to_string().contains("no scripted event available"),
        "error should preserve scripted fallback read message"
    );
}

#[test]
fn run_loop_propagates_poll_errors() {
    let model = super::super::tests::support::sample_model(1, 1);
    let mut state = AppState::new(&model);
    let mut terminal =
        ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
    let events = ScriptedEventSource::new(
        vec![Err(io::Error::new(
            ErrorKind::BrokenPipe,
            "scripted poll failure",
        ))],
        vec![],
    );

    let error = run_loop(&mut terminal, &model, &mut state, &events)
        .expect_err("poll errors should stop the run loop");
    assert!(
        error.to_string().contains("scripted poll failure"),
        "error should preserve poll failure details"
    );
}

#[test]
fn crossterm_event_source_poll_is_callable() {
    let source = CrosstermEventSource;
    let _ = source.poll(Duration::from_millis(0));
}
