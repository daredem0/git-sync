//! Terminal UI composition, rendering, and interaction state.

mod diff;
mod format;
mod input;
mod model;
mod render;
mod runtime;
mod state;
mod syntax;
mod types;

/// Runs the interactive terminal audit UI.
pub use runtime::run;

#[cfg(test)]
mod tests;
