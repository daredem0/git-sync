//! Unit tests for state diff scroll tests.

// Focus: AppState diff scrolling bounds and line-number bookkeeping in rendered patch views.

use super::super::types::{DiffViewState, PatchLineKind};
use super::support::*;
use ratatui::text::Line;

// Verifies that diff scrolling clamps to valid bounds and never underflows/overflows.
#[test]
fn diff_scroll_is_bounded() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.diff_view = Some(DiffViewState {
        commit_index: 0,
        commit_total: 1,
        file_index: 0,
        commit_id: git2::Oid::from_str("1111111111111111111111111111111111111111")
            .expect("valid oid"),
        commit_subject: "subject".to_string(),
        file_path: "f.rs".to_string(),
        syntax_name: "Rust".to_string(),
        lines: vec![
            Line::from("line 1"),
            Line::from("line 2"),
            Line::from("line 3"),
        ],
        max_line_width: 12,
        scroll_y: 0,
        scroll_x: 0,
    });

    state.scroll_diff_up(100);
    state.scroll_diff_left(100);
    assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_y, 0);
    assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_x, 0);

    state.scroll_diff_down(100);
    state.scroll_diff_right(100);
    assert_eq!(
        state.diff_view.as_ref().expect("diff view").scroll_y,
        2,
        "vertical diff scroll should clamp to last line index"
    );
    assert_eq!(
        state.diff_view.as_ref().expect("diff view").scroll_x,
        11,
        "horizontal diff scroll should clamp to max_line_width - 1"
    );

    state.reset_diff_scroll();
    assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_y, 0);
    assert_eq!(state.diff_view.as_ref().expect("diff view").scroll_x, 0);
}

// Verifies that line-number column tracking stays consistent across context/delete/add sequences.
#[test]
fn line_number_columns_tracks_old_and_new_counters() {
    let mut old = Some(10usize);
    let mut new = Some(20usize);

    let context =
        super::super::diff::line_number_columns(PatchLineKind::Context, &mut old, &mut new);
    assert_eq!(context, ("10".to_string(), "20".to_string()));
    assert_eq!(old, Some(11));
    assert_eq!(new, Some(21));

    let deleted =
        super::super::diff::line_number_columns(PatchLineKind::Deleted, &mut old, &mut new);
    assert_eq!(deleted, ("11".to_string(), "".to_string()));
    assert_eq!(old, Some(12));
    assert_eq!(new, Some(21));

    let added = super::super::diff::line_number_columns(PatchLineKind::Added, &mut old, &mut new);
    assert_eq!(added, ("".to_string(), "21".to_string()));
    assert_eq!(old, Some(12));
    assert_eq!(new, Some(22));
}
