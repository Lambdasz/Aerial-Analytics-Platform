//! Module 10 — Simple Temporal Change Analysis.
//!
//! Provides cross-module data contracts for before-and-after comparison of
//! vegetation coverage, land-cover transitions, and tree-count deltas between
//! observation periods.
//!
//! Module layout:
//! - [`analysis`] — pure analysis functions for 10.1 - 10.7.
//! - [`observations`] — adapter from Module 1.4 stored images to observations.
//! - [`coverage`] — vegetation coverage measurement used by 10.3.
//! - [`commands`] — Tauri entry points that read Module 1.4 data and run the
//!   analysis.
//! - [`functions`] — cross-module DTO contracts (10.7 → Module 3.4 and
//!   10.8 → Module 11.5).
//! - [`comparisons`] — persisted before/after comparison history.
//! - [`spatial_change`] — 10.7 spatial change regions between two dates.
//! - [`visualization`] — 10.8 payload and analytical summary for the UI.

pub mod analysis;
pub mod commands;
pub mod comparisons;
pub mod coverage;
pub mod functions;
pub mod observations;
pub mod spatial_change;
pub mod visualization;
