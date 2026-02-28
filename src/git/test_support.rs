//! Test-only helper exports for the git module test suite.

pub(crate) use super::archive::{
    extract_bundle_archive, remove_file_if_exists, resolve_patch_sidecar_path, write_zip_archive,
};
pub(crate) use super::bundle::{
    is_head_already_applied, open_payload_session_with_resolve_mode,
    verify_pack_payload_for_bundle_input, verify_pack_payload_for_bundle_input_with_resolve_mode,
};
pub(crate) use super::diff::collect_diff_entries;
pub(crate) use super::metadata::{
    load_bundle_metadata_from_path, verify_bundle_metadata_against_repo,
    verify_bundle_metadata_integrity, verify_bundle_metadata_integrity_input,
};
pub(crate) use super::types::CreateBundleAuditPatchSidecar;
pub(crate) use super::util::{bundle_version_code, oid_or_none, path_to_string, sha256_hex};
pub(crate) use crate::app::AppConfig;
