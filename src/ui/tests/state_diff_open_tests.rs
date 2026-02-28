//! Unit tests for state diff open tests.

// Focus: AppState diff-opening behavior for normal, out-of-context, and non-text commit file selections.

use super::support::*;

// Verifies that opening a diff from non-commit context does not create an active diff view.
#[test]
fn open_selected_diff_noop_outside_commit_context() {
    let model = sample_model(1, 1);
    let mut state = super::super::types::AppState::new(&model);
    state.open_selected_diff(&model);
    assert!(
        state.diff_view.is_none(),
        "diff view should remain closed when not on a commit page"
    );
}

// Verifies that opening a selected diff on a commit page creates a populated diff view model.
#[test]
fn open_selected_diff_creates_diff_view_for_selected_file() {
    let fixture = create_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.next_page(&model);
    let commit_index = 0usize;
    let target_index = fixture.entries[commit_index]
        .files
        .iter()
        .position(|file| file.path == "f.rs")
        .expect("fixture commit should contain f.rs");
    state.selected_file_indices[0][commit_index] = target_index;

    state.open_selected_diff(&model);

    let diff_view = state
        .diff_view
        .as_ref()
        .expect("diff view should be opened for selected file");
    assert_eq!(
        diff_view.commit_total,
        fixture.entries.len(),
        "diff view should carry total commit count for header rendering"
    );
    assert_eq!(
        diff_view.file_path, "f.rs",
        "diff view should open the selected commit file path"
    );
    assert!(
        !diff_view.lines.is_empty(),
        "diff view should include rendered patch lines"
    );
    assert!(
        diff_view.syntax_name.contains("Rust"),
        "syntax detection should identify Rust for .rs files"
    );
    assert!(
        state.action_message.is_none(),
        "opening valid diff should not set an error/action message"
    );
}

// Verifies that opening a non-text changed path (for example symlink) is a no-op and does not show an error.
#[test]
fn open_selected_diff_noop_for_non_text_changed_file() {
    let fixture = create_non_text_diff_fixture();
    let model = build_model_from_fixture(&fixture);
    let mut state = super::super::types::AppState::new(&model);
    state.next_page(&model);

    let commit_index = 0usize;
    let target_index = fixture.entries[commit_index]
        .files
        .iter()
        .position(|file| file.path == "link-to-f")
        .expect("fixture commit should contain symlink path");
    state.selected_file_indices[0][commit_index] = target_index;

    state.open_selected_diff(&model);

    assert!(
        state.diff_view.is_none(),
        "diff view should stay closed for non-text changed paths"
    );
    assert!(
        state.action_message.is_none(),
        "non-text path open attempts should not surface an error banner"
    );
}
