//! Bundle-oriented operations: create, inspect, receive, and commit patch views.

mod create;
mod inspect;
mod parse;
mod payload;
mod receive;

pub use create::{create_bundle, create_bundle_with_options, remove_unarchived_bundle_artifacts};
pub use inspect::inspect_bundle;
pub use payload::{
    PayloadSession, build_payload_audit_document_for_bundle_input_with_options,
    collect_payload_audit_for_bundle_input_with_resolve_mode,
    collect_payload_object_detail_for_bundle_input, collect_payload_object_detail_for_session,
    open_payload_session, payload_audit_from_session,
};

#[cfg(test)]
pub(crate) use payload::open_payload_session_with_resolve_mode;
#[cfg(test)]
pub(crate) use payload::verify_pack_payload_for_bundle_input;
#[cfg(test)]
pub(crate) use payload::verify_pack_payload_for_bundle_input_with_resolve_mode;
pub use receive::{
    collect_commit_file_patch_for_bundle_input, collect_head_audit_entries_for_bundle_input,
    receive_bundle_input, receive_bundle_input_with_options,
};

#[cfg(test)]
pub(crate) use receive::is_head_already_applied;
