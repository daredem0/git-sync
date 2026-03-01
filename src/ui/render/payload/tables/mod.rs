// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Payload table rendering module wiring and exports.
//!
//! Part of the read-only review UI that projects verified evidence for operators.
//! Keeps interaction and rendering concerns separate from proof computation.

mod entries;
mod objects;
mod transport;

pub(super) use entries::render_entries_table;
pub(super) use objects::render_objects_table;
pub(super) use transport::render_transport_entries_table;
