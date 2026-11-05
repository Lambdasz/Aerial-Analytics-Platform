//! # Module 11: Aerial Analytics Dashboard & Reporting
//!
//! Compatibility entry for the original Module 11 reporting contract. The real
//! implementation now lives in [`crate::modules::module_11`]:
//!
//! - [`crate::modules::module_11::dashboard`] — read-only project dashboard and
//!   plugin result explorer queries.
//! - [`crate::modules::module_11::exports`] — CSV, GeoJSON and JSON exports.
//! - [`crate::modules::module_11::report`] — the printable HTML report.
//!
//! This module only keeps the two commands the original contract promised
//! ([`commands::generate_report`], [`commands::export_data`]) and forwards them
//! to those implementations. The PDF/XLSX formats the scaffold sketched are not
//! implemented: the report is HTML (printable to PDF) and exports are CSV,
//! GeoJSON and JSON.
//!
//! The previous stub tree (`aggregator`, `report`, `export`, `types`,
//! `RULES_REPORTING.md`) was removed: its local duplicates of the Module 9/10
//! types were superseded when those modules shipped their own data model.

pub mod commands;
