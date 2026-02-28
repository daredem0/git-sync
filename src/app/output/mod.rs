//! Non-interactive CLI output rendering helpers.

mod json;
mod kind;
mod layout;
mod sections;
mod table;

pub use json::render_payload_audit_json;
pub use table::render_payload_audit_table;

#[cfg(test)]
mod tests;
