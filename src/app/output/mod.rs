// SPDX-FileCopyrightText: 2026 Florian Leuze
// SPDX-License-Identifier: Apache-2.0

//! Output module wiring for table, section, and JSON emitters.
//!
//! Part of the application orchestration layer that translates CLI intent into domain calls.
//! Keeps command flow boundaries explicit and user-facing output predictable.

mod json;
mod kind;
mod layout;
mod sections;
mod table;

pub use json::render_payload_audit_json;
pub use table::render_payload_audit_table;

#[cfg(test)]
mod tests;
