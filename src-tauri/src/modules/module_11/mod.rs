//! Module 11 — dashboard and result export (P1–P5 scope).
//!
//! Read-only aggregation for the analytics dashboard plus file exports for GIS
//! and statistical use. The heavier `reporting/` scaffolding (PDF reports) is
//! intentionally not wired in yet.

pub mod dashboard;
pub mod exports;
pub mod report;

pub use dashboard::{build_project_dashboard, list_plugin_results, ProjectDashboard};
