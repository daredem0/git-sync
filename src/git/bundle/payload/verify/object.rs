//! Object/OID mapping helpers for PACK verification.

use crate::git::digest::sha1_hex;
use crate::git::types::{PackEntryKind, PayloadObjectKind};
use anyhow::{Result, bail};

#[derive(Debug, Clone)]
pub(super) struct ParsedPackObject {
    pub(super) kind: PayloadObjectKind,
    pub(super) content: Vec<u8>,
}

/// Returns the canonical object id for `(kind, content)` as SHA-1.
pub(super) fn object_oid_for_content(kind: PayloadObjectKind, content: &[u8]) -> Result<git2::Oid> {
    let type_name = match kind {
        PayloadObjectKind::Commit => "commit",
        PayloadObjectKind::Tree => "tree",
        PayloadObjectKind::Blob => "blob",
        PayloadObjectKind::Tag => "tag",
        PayloadObjectKind::Unknown => bail!("cannot hash unknown pack object kind"),
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(format!("{type_name} {}\0", content.len()).as_bytes());
    bytes.extend_from_slice(content);
    let digest_hex = sha1_hex(&bytes)?;
    Ok(git2::Oid::from_str(&digest_hex)?)
}

/// Loads one baseline object by OID for external ref-delta base resolution.
pub(super) fn load_parsed_object_from_odb(
    odb: &git2::Odb<'_>,
    oid: git2::Oid,
) -> Result<ParsedPackObject> {
    let object = odb.read(oid)?;
    let kind = payload_kind_from_git(object.kind());
    if matches!(kind, PayloadObjectKind::Unknown) {
        bail!("baseline base object has unsupported type for delta resolution: {oid}");
    }
    Ok(ParsedPackObject {
        kind,
        content: object.data().to_vec(),
    })
}

/// Converts one pack entry kind into payload object kind.
pub(super) fn pack_entry_kind_to_payload_kind(kind: PackEntryKind) -> PayloadObjectKind {
    match kind {
        PackEntryKind::Commit => PayloadObjectKind::Commit,
        PackEntryKind::Tree => PayloadObjectKind::Tree,
        PackEntryKind::Blob => PayloadObjectKind::Blob,
        PackEntryKind::Tag => PayloadObjectKind::Tag,
        PackEntryKind::OfsDelta | PackEntryKind::RefDelta => PayloadObjectKind::Unknown,
    }
}

fn payload_kind_from_git(kind: git2::ObjectType) -> PayloadObjectKind {
    match kind {
        git2::ObjectType::Commit => PayloadObjectKind::Commit,
        git2::ObjectType::Tree => PayloadObjectKind::Tree,
        git2::ObjectType::Blob => PayloadObjectKind::Blob,
        git2::ObjectType::Tag => PayloadObjectKind::Tag,
        _ => PayloadObjectKind::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        load_parsed_object_from_odb, object_oid_for_content, pack_entry_kind_to_payload_kind,
    };
    use crate::git::{PackEntryKind, PayloadObjectKind};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_repo_dir(prefix: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("git-sync-{prefix}-{nanos}"))
    }

    #[test]
    fn object_oid_for_content_matches_git_blob_hash_and_rejects_unknown_kind() {
        let repo_dir = temp_repo_dir("verify-object-oid");
        std::fs::create_dir_all(&repo_dir).expect("must create temporary repo directory");
        let repo = git2::Repository::init(&repo_dir).expect("must initialize temporary repository");
        let blob_bytes = b"hello world\n";
        let expected = repo
            .blob(blob_bytes)
            .expect("must write blob into temporary repository");

        let actual = object_oid_for_content(PayloadObjectKind::Blob, blob_bytes)
            .expect("blob oid should be computed");
        assert_eq!(
            actual, expected,
            "computed blob oid should match git's canonical object id"
        );

        let error = object_oid_for_content(PayloadObjectKind::Unknown, blob_bytes)
            .expect_err("unknown object kind should be rejected");
        assert!(
            error
                .to_string()
                .contains("cannot hash unknown pack object kind"),
            "error should report unsupported unknown object kind"
        );

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn load_parsed_object_from_odb_reads_blob_kind_and_content() {
        let repo_dir = temp_repo_dir("verify-object-load");
        std::fs::create_dir_all(&repo_dir).expect("must create temporary repo directory");
        let repo = git2::Repository::init(&repo_dir).expect("must initialize temporary repository");
        let blob_bytes = b"blob-bytes";
        let blob_oid = repo
            .blob(blob_bytes)
            .expect("must write blob into temporary repository");
        let odb = repo.odb().expect("must open object database");

        let parsed = load_parsed_object_from_odb(&odb, blob_oid)
            .expect("blob object should be loadable from baseline odb");
        assert_eq!(parsed.kind, PayloadObjectKind::Blob);
        assert_eq!(parsed.content, blob_bytes);

        let _ = std::fs::remove_dir_all(repo_dir);
    }

    #[test]
    fn pack_entry_kind_to_payload_kind_maps_delta_entries_to_unknown() {
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::Commit),
            PayloadObjectKind::Commit
        );
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::Tree),
            PayloadObjectKind::Tree
        );
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::Blob),
            PayloadObjectKind::Blob
        );
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::Tag),
            PayloadObjectKind::Tag
        );
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::OfsDelta),
            PayloadObjectKind::Unknown
        );
        assert_eq!(
            pack_entry_kind_to_payload_kind(PackEntryKind::RefDelta),
            PayloadObjectKind::Unknown
        );
    }
}
