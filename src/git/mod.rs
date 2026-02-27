//! Git-domain operations for bundle creation, receive, diffing, and metadata.

mod archive;
mod bundle;
mod context;
mod diff;
mod manifest;
mod metadata;
mod range;
mod types;
mod util;

#[allow(unused_imports)]
pub use bundle::{
    collect_commit_audit_entries_for_bundle_input, collect_commit_file_patch_for_bundle_input,
    create_bundle, create_bundle_with_options, inspect_bundle, receive_bundle_input,
    receive_bundle_input_with_options, remove_unarchived_bundle_artifacts,
};
#[allow(unused_imports)]
pub use context::open_context;
#[allow(unused_imports)]
pub use diff::collect_changed_files;
#[allow(unused_imports)]
pub use manifest::{render_manifest, render_manifest_json};
#[allow(unused_imports)]
pub use metadata::{
    collect_changed_files_from_bundle_input, verify_bundle_metadata_against_repo,
    verify_bundle_metadata_against_repo_input,
};
#[allow(unused_imports)]
pub use range::resolve_repo_audit_range;
#[allow(unused_imports)]
pub use types::{
    BundleHead, BundleInspection, BundleVersion, ChangeStatus, ChangedFile, CommitAuditEntry,
    CommitAuditIdentity, CreateBundleOptions, CreateBundleResult, FileLineStat, OpenContext,
    ReceiveBundleOptions, ReceiveBundleResult, RepoAuditRange,
};

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use crate::app::AppConfig;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use archive::{
    bundle_archive_path, caudit_sidecar_path, extract_bundle_archive, is_zip_bundle_input_path,
    patch_sidecar_path, remove_file_if_exists, resolve_patch_sidecar_path, write_zip_archive,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use bundle::is_head_already_applied;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use diff::collect_diff_entries;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use metadata::{
    collect_changed_files_for_metadata, collect_commit_chain_for_metadata,
    load_bundle_metadata_from_input, load_bundle_metadata_from_path, signature_to_audit_signature,
    verify_bundle_metadata_integrity, verify_bundle_metadata_integrity_input, write_patch_sidecar,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use types::{
    CreateBundleAuditChangedFile, CreateBundleAuditCommit, CreateBundleAuditHead,
    CreateBundleAuditMetadata, CreateBundleAuditPatchSidecar, CreateBundleAuditSignature,
    DiffEntry,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use util::{
    bundle_version_code, current_hostname, current_unix_timestamp_secs, current_username,
    oid_or_none, oid_to_str, parse_optional_oid, parse_status_code, path_to_string, sha256_hex,
    status_code,
};

#[cfg(test)]
mod tests;
