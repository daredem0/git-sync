mod collect;
mod load;
mod patch;
mod verify;

pub use load::collect_changed_files_from_bundle_input;
pub use verify::{verify_bundle_metadata_against_repo, verify_bundle_metadata_against_repo_input};

#[allow(unused_imports)]
pub(crate) use collect::{
    collect_changed_files_for_metadata, collect_commit_chain_for_metadata,
    signature_to_audit_signature,
};
#[allow(unused_imports)]
pub(crate) use load::{load_bundle_metadata_from_input, load_bundle_metadata_from_path};
pub(crate) use patch::write_patch_sidecar;
#[allow(unused_imports)]
pub(crate) use verify::{verify_bundle_metadata_integrity, verify_bundle_metadata_integrity_input};
