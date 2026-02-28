//! Payload session bootstrap and repo-policy checks.

use crate::git::types::{PayloadAudit, PayloadResolveMode};
use crate::git::util::sha256_hex;
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::path::Path;

use super::{PayloadSession, context, input, parse};

pub(super) fn open_payload_session_with_resolve_mode_impl(
    bundle_input_path: &Path,
    repo_path: &Path,
    resolve_mode: PayloadResolveMode,
) -> Result<PayloadSession> {
    ensure_supported_repo_object_format(repo_path)?;
    let loaded = input::load_payload_input(bundle_input_path)?;

    let baseline_repo = if matches!(resolve_mode, PayloadResolveMode::Baseline) {
        Some(git2::Repository::open(repo_path)?)
    } else {
        None
    };
    let resolve_odb = baseline_repo
        .as_ref()
        .map(git2::Repository::odb)
        .transpose()?;

    let parsed_bundle = parse::parse_bundle_payload(&loaded.bundle_bytes)?;
    let inspection = parsed_bundle.inspection.clone();
    let verification = super::verify_pack_payload_with_ledger_and_baseline_odb(
        parsed_bundle.pack_data,
        resolve_odb.as_ref(),
    )
    .map_err(anyhow::Error::from)?;

    let materialized_store_by_oid = verification
        .materialized_store
        .objects
        .iter()
        .cloned()
        .map(|entry| (entry.oid, entry))
        .collect::<HashMap<_, _>>();

    let (reachable, context_map, blob_paths_by_oid) =
        context::collect_reachability_context_from_materialized(
            &inspection.heads,
            &materialized_store_by_oid,
        );
    let objects = context::collect_payload_objects_from_materialized_index(
        &verification.materialized_index,
        &reachable,
        &context_map,
    );

    let payload = PayloadAudit {
        bundle_version: inspection.version,
        heads: inspection.heads.clone(),
        transport_entries: loaded.transport_entries,
        pack_proof: verification.proof.clone(),
        entry_ledger: verification.ledger,
        objects,
    };

    Ok(PayloadSession {
        inspection,
        payload,
        materialized_store_by_oid,
        blob_paths_by_oid,
        bundle_path: loaded.bundle_name,
        bundle_size_bytes: loaded.bundle_bytes.len() as u64,
        bundle_sha256: sha256_hex(&loaded.bundle_bytes)?,
    })
}

fn ensure_supported_repo_object_format(repo_path: &Path) -> Result<()> {
    let repo = git2::Repository::open(repo_path)?;
    let config = repo.config()?;
    let object_format = config
        .get_string("extensions.objectformat")
        .unwrap_or_else(|_| "sha1".to_string())
        .to_ascii_lowercase();
    if object_format != "sha1" {
        bail!(
            "unsupported repository object format '{}' at {}: payload audit currently supports only sha1",
            object_format,
            repo_path.display()
        );
    }
    Ok(())
}
