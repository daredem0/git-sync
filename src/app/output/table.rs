// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Output formatting logic for table views.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

use crate::git::PayloadAudit;

use super::layout::compute_table_widths;
use super::sections::{
    append_objects_section, append_pack_proof_section, append_transport_section,
};

/// Renders non-interactive payload audit as a human-readable aligned table.
pub fn render_payload_audit_table(payload: &PayloadAudit) -> String {
    let widths = compute_table_widths(payload);
    let mut out = String::new();

    append_pack_proof_section(&mut out, payload);
    append_transport_section(&mut out, payload, &widths);
    append_objects_section(&mut out, payload, &widths);

    out
}
