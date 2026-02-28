//! Width/layout calculation helpers for table rendering.

use crate::git::PayloadAudit;

use super::kind::payload_kind_label;

pub(super) const OID_HEADER: &str = "OID";
pub(super) const TYPE_HEADER: &str = "TYPE";
pub(super) const SIZE_HEADER: &str = "SIZE";
pub(super) const REACHABLE_HEADER: &str = "REACHABLE";
pub(super) const TRANSPORT_NAME_HEADER: &str = "NAME";
pub(super) const TRANSPORT_SIZE_HEADER: &str = "SIZE";
pub(super) const TRANSPORT_SHA_HEADER: &str = "SHA256";

pub(super) struct TableWidths {
    pub(super) oid: usize,
    pub(super) kind: usize,
    pub(super) size: usize,
    pub(super) reachable: usize,
    pub(super) transport_name: usize,
    pub(super) transport_size: usize,
}

pub(super) fn compute_table_widths(payload: &PayloadAudit) -> TableWidths {
    let oid = std::cmp::max(
        OID_HEADER.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.oid.to_string().len())
            .max()
            .unwrap_or(0),
    );

    let kind = std::cmp::max(
        TYPE_HEADER.len(),
        payload
            .objects
            .iter()
            .map(|entry| payload_kind_label(entry.kind).len())
            .max()
            .unwrap_or(0),
    );

    let size = std::cmp::max(
        SIZE_HEADER.len(),
        payload
            .objects
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );

    let reachable = std::cmp::max(REACHABLE_HEADER.len(), 9);

    let transport_name = std::cmp::max(
        TRANSPORT_NAME_HEADER.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.name.len())
            .max()
            .unwrap_or(0),
    );

    let transport_size = std::cmp::max(
        TRANSPORT_SIZE_HEADER.len(),
        payload
            .transport_entries
            .iter()
            .map(|entry| entry.size_bytes.to_string().len())
            .max()
            .unwrap_or(0),
    );

    TableWidths {
        oid,
        kind,
        size,
        reachable,
        transport_name,
        transport_size,
    }
}
