//! mihomo-subscription library crate.
//!
//! Exposes the building blocks of the service so integration tests under
//! `tests/` can exercise them directly. See `docs/` for the target design.

pub mod app;
pub mod auth;
pub mod converter;
pub mod db;
pub mod error;
pub mod fetch;
pub mod mask;
pub mod profiles;
pub mod settings;
pub mod ssrf;
pub mod util;
pub mod yaml;
