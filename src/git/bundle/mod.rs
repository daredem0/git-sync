//! Bundle-oriented operations: create, inspect, receive, and commit patch views.

mod create;
mod inspect;
mod receive;

pub use create::{create_bundle, create_bundle_with_options, remove_unarchived_bundle_artifacts};
pub use inspect::inspect_bundle;
pub use receive::{
    collect_commit_audit_entries_for_bundle_input, collect_commit_file_patch_for_bundle_input,
    receive_bundle_input, receive_bundle_input_with_options,
};

#[cfg(test)]
pub(crate) use receive::is_head_already_applied;
