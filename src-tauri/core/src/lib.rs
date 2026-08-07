//! Everything Daybook knows how to do to a vault, with no UI attached.
//!
//! Split out of the app so a second binary — the MCP server — can share it
//! without dragging in Tauri. That is not only about build time: while these
//! modules lived inside the Tauri package, adding a second `[[bin]]` made the
//! bundler pick the wrong one and ship a headless stdio server as the app.

pub mod backfill;
pub mod config;
pub mod datetime;
pub mod entries;
pub mod trash;
pub mod vault;
