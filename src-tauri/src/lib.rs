//! Traygent core: the IPC data model, the agent domain (catalog, settings, recent sessions,
//! detection, launch builder, `state.json`) and the platform-independent rules of the shell.
//! The browser preview serves its own demo snapshot (`src/lib/demo.ts`).

pub mod catalog;
pub mod detect;
pub mod github;
pub mod launch;
pub mod models;
pub mod popup;
pub mod ports;
pub mod recents;
pub mod repo;
pub mod service;
pub mod settings;
pub mod spawn;
pub mod startup;
pub mod store;

#[cfg(test)]
pub(crate) mod testing;

pub use models::*;

#[cfg(feature = "updates")]
pub mod updates;
