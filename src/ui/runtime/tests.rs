// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Module-scoped tests for ui/runtime.rs.
//!
//! Part of the git-sync regression suite for command, domain, and UI correctness.
//! Protects behavior and proof-relevant invariants with focused automated checks.

use super::*;
use crate::app::AppConfig;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::backend::TestBackend;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

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

struct TestRuntimeOps {
    terminal: RefCell<Option<ratatui::Terminal<TestBackend>>>,
    cleanup_calls: RefCell<usize>,
}

impl TestRuntimeOps {
    fn new(width: u16, height: u16) -> Self {
        let terminal =
            ratatui::Terminal::new(TestBackend::new(width, height)).expect("must create terminal");
        Self {
            terminal: RefCell::new(Some(terminal)),
            cleanup_calls: RefCell::new(0),
        }
    }

    fn cleanup_calls(&self) -> usize {
        *self.cleanup_calls.borrow()
    }
}

impl RuntimeOps for TestRuntimeOps {
    type B = TestBackend;

    fn setup_terminal(&self) -> Result<ratatui::Terminal<Self::B>> {
        self.terminal
            .borrow_mut()
            .take()
            .ok_or_else(|| anyhow::anyhow!("scripted setup terminal already consumed"))
    }

    fn cleanup_terminal(&self, _terminal: &mut ratatui::Terminal<Self::B>) {
        *self.cleanup_calls.borrow_mut() += 1;
    }
}

struct FailingRuntimeOps;

impl RuntimeOps for FailingRuntimeOps {
    type B = TestBackend;

    fn setup_terminal(&self) -> Result<ratatui::Terminal<Self::B>> {
        Err(anyhow::anyhow!("scripted setup failure"))
    }

    fn cleanup_terminal(&self, _terminal: &mut ratatui::Terminal<Self::B>) {
        panic!("cleanup should not run when setup fails");
    }
}

static SETUP_ENABLE_RAW_CALLS: AtomicUsize = AtomicUsize::new(0);
static SETUP_ENTER_ALT_CALLS: AtomicUsize = AtomicUsize::new(0);
static CLEANUP_DISABLE_RAW_CALLS: AtomicUsize = AtomicUsize::new(0);
static CLEANUP_LEAVE_ALT_CALLS: AtomicUsize = AtomicUsize::new(0);
static CLEANUP_SHOW_CURSOR_CALLS: AtomicUsize = AtomicUsize::new(0);

fn setup_enable_raw_stub() -> io::Result<()> {
    SETUP_ENABLE_RAW_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn setup_enter_alt_stub(_stdout: &mut io::Stdout) -> io::Result<()> {
    SETUP_ENTER_ALT_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn setup_enter_alt_fail_stub(_stdout: &mut io::Stdout) -> io::Result<()> {
    Err(io::Error::other("scripted enter-alt failure"))
}

fn setup_build_test_terminal(_stdout: io::Stdout) -> Result<ratatui::Terminal<TestBackend>> {
    ratatui::Terminal::new(TestBackend::new(100, 30)).map_err(Into::into)
}

fn cleanup_disable_raw_stub() -> io::Result<()> {
    CLEANUP_DISABLE_RAW_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn cleanup_leave_alt_test_stub(_backend: &mut TestBackend) -> io::Result<()> {
    CLEANUP_LEAVE_ALT_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn cleanup_show_cursor_test_stub(_terminal: &mut ratatui::Terminal<TestBackend>) -> io::Result<()> {
    CLEANUP_SHOW_CURSOR_CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

fn run_override_ok(_config: &AppConfig) -> anyhow::Result<()> {
    Ok(())
}

fn run_override_err(_config: &AppConfig) -> anyhow::Result<()> {
    Err(anyhow::anyhow!("scripted run override failure"))
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
fn setup_crossterm_terminal_with_uses_injected_setup_operations() {
    SETUP_ENABLE_RAW_CALLS.store(0, Ordering::SeqCst);
    SETUP_ENTER_ALT_CALLS.store(0, Ordering::SeqCst);

    let terminal = setup_terminal_with(
        setup_enable_raw_stub,
        setup_enter_alt_stub,
        setup_build_test_terminal,
    );
    assert!(
        terminal.is_ok(),
        "setup helper should build terminal when injected setup hooks succeed"
    );
    assert_eq!(
        SETUP_ENABLE_RAW_CALLS.load(Ordering::SeqCst),
        1,
        "setup helper should invoke injected raw-mode hook once"
    );
    assert_eq!(
        SETUP_ENTER_ALT_CALLS.load(Ordering::SeqCst),
        1,
        "setup helper should invoke injected enter-alt hook once"
    );
}

#[test]
fn setup_crossterm_terminal_with_propagates_enter_alt_failures() {
    let error = setup_terminal_with(
        setup_enable_raw_stub,
        setup_enter_alt_fail_stub,
        setup_build_test_terminal,
    )
    .expect_err("enter-alt setup failures should bubble up");
    assert!(
        error.to_string().contains("scripted enter-alt failure"),
        "setup helper should preserve enter-alt failure detail"
    );
}

#[test]
fn cleanup_crossterm_terminal_with_uses_injected_cleanup_operations() {
    CLEANUP_DISABLE_RAW_CALLS.store(0, Ordering::SeqCst);
    CLEANUP_LEAVE_ALT_CALLS.store(0, Ordering::SeqCst);
    CLEANUP_SHOW_CURSOR_CALLS.store(0, Ordering::SeqCst);

    let mut terminal = setup_terminal_with(
        setup_enable_raw_stub,
        setup_enter_alt_stub,
        setup_build_test_terminal,
    )
    .expect("terminal setup should succeed for cleanup helper test");
    cleanup_terminal_with(
        &mut terminal,
        cleanup_disable_raw_stub,
        cleanup_leave_alt_test_stub,
        cleanup_show_cursor_test_stub,
    );

    assert_eq!(
        CLEANUP_DISABLE_RAW_CALLS.load(Ordering::SeqCst),
        1,
        "cleanup helper should invoke injected disable-raw hook once"
    );
    assert_eq!(
        CLEANUP_LEAVE_ALT_CALLS.load(Ordering::SeqCst),
        1,
        "cleanup helper should invoke injected leave-alt hook once"
    );
    assert_eq!(
        CLEANUP_SHOW_CURSOR_CALLS.load(Ordering::SeqCst),
        1,
        "cleanup helper should invoke injected show-cursor hook once"
    );
}

#[test]
fn crossterm_screen_helpers_are_callable() {
    let mut sink = Vec::<u8>::new();
    let _ = enter_alternate_screen(&mut sink);

    let mut backend = ratatui::backend::CrosstermBackend::new(Vec::<u8>::new());
    let _ = leave_alternate_screen(&mut backend);

    let mut terminal =
        ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
    let _ = show_terminal_cursor(&mut terminal);
}

#[test]
fn crossterm_event_source_poll_is_callable() {
    let source = CrosstermEventSource::default();
    let _ = source.poll(Duration::from_millis(0));
}

#[test]
fn crossterm_event_source_read_uses_injected_reader() {
    fn poll_stub(_timeout: Duration) -> io::Result<bool> {
        Ok(true)
    }

    fn read_stub() -> io::Result<Event> {
        Ok(Event::Resize(80, 24))
    }

    let source = CrosstermEventSource {
        poll_fn: poll_stub,
        read_fn: read_stub,
    };

    assert!(
        source
            .poll(Duration::from_millis(0))
            .expect("stub poll should succeed"),
        "stub poll result should be returned by event source"
    );
    assert!(
        matches!(
            source.read().expect("stub read should succeed"),
            Event::Resize(80, 24)
        ),
        "stub read result should be returned by event source"
    );
}

#[test]
fn run_with_loaded_model_exits_on_quit_key() {
    let model = super::super::tests::support::sample_model(1, 1);
    let mut terminal =
        ratatui::Terminal::new(TestBackend::new(100, 30)).expect("must create test terminal");
    let events = ScriptedEventSource::new(
        vec![Ok(true)],
        vec![Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )))],
    );

    let result = run_with_loaded_model(&mut terminal, model, &events);
    assert!(
        result.is_ok(),
        "run_with_loaded_model should stop cleanly on quit key"
    );
}

#[test]
fn run_with_runtime_performs_cleanup_after_successful_loop() {
    let fixture = crate::ui::tests::support::create_diff_fixture();
    let config = AppConfig {
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };
    let runtime = TestRuntimeOps::new(100, 30);
    let events = ScriptedEventSource::new(
        vec![Ok(true)],
        vec![Ok(Event::Key(KeyEvent::new(
            KeyCode::Char('q'),
            KeyModifiers::NONE,
        )))],
    );

    let result = run_with_runtime(&config, &runtime, &events);
    assert!(
        result.is_ok(),
        "runtime wrapper should return ok when scripted loop exits via quit key"
    );
    assert_eq!(
        runtime.cleanup_calls(),
        1,
        "runtime wrapper should perform terminal cleanup exactly once"
    );
}

#[test]
fn run_with_runtime_performs_cleanup_when_loop_returns_error() {
    let fixture = crate::ui::tests::support::create_diff_fixture();
    let config = AppConfig {
        repo_path: fixture.receiver_dir.clone(),
        bundle_path: fixture.bundle_archive_path.clone(),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };
    let runtime = TestRuntimeOps::new(100, 30);
    let events = ScriptedEventSource::new(
        vec![Ok(true)],
        vec![Err(io::Error::new(
            ErrorKind::ConnectionReset,
            "scripted loop read failure",
        ))],
    );

    let error =
        run_with_runtime(&config, &runtime, &events).expect_err("runtime wrapper should fail");
    assert!(
        error.to_string().contains("scripted loop read failure"),
        "runtime wrapper should preserve loop error details"
    );
    assert_eq!(
        runtime.cleanup_calls(),
        1,
        "runtime wrapper should still clean up terminal after loop errors"
    );
}

#[test]
fn run_entrypoint_uses_test_override_success_path() {
    set_test_run_override(Some(run_override_ok));
    let config = AppConfig {
        repo_path: PathBuf::from("unused-repo"),
        bundle_path: PathBuf::from("unused-bundle"),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };
    let result = run(&config);
    set_test_run_override(None);

    assert!(
        result.is_ok(),
        "run entrypoint should return override success result in tests"
    );
}

#[test]
fn run_entrypoint_uses_test_override_error_path() {
    set_test_run_override(Some(run_override_err));
    let config = AppConfig {
        repo_path: PathBuf::from("unused-repo"),
        bundle_path: PathBuf::from("unused-bundle"),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };
    let error = run(&config).expect_err("run override error should bubble up");
    set_test_run_override(None);

    assert!(
        error.to_string().contains("scripted run override failure"),
        "run entrypoint should preserve test override failure details"
    );
}

#[test]
fn run_with_runtime_propagates_setup_error() {
    let config = AppConfig {
        repo_path: PathBuf::from("unused-repo"),
        bundle_path: PathBuf::from("unused-bundle"),
        base_ref: "refs/heads/base".to_string(),
        tip_ref: None,
    };
    let runtime = FailingRuntimeOps;
    let events = ScriptedEventSource::new(vec![], vec![]);

    let error =
        run_with_runtime(&config, &runtime, &events).expect_err("setup failures should bubble up");
    assert!(
        error.to_string().contains("scripted setup failure"),
        "runtime wrapper should preserve setup failure details"
    );
}

#[test]
fn render_loading_screen_renders_expected_help_text() {
    let output = crate::ui::tests::support::render_and_capture_text(100, 20, |frame| {
        render_loading_screen(frame);
    });
    assert!(
        output.contains("Loading audit view...")
            && output.contains("This can take a while on large bundles.")
            && output.contains("git-sync audit --format table"),
        "loading screen should render loading hint and non-interactive tip text"
    );
}
