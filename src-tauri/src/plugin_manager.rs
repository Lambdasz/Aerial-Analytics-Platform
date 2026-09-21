//! Plugin System & Extension Manager (Module 2).
//!
//! Provides a plugin-based architecture so that analytical capabilities can be
//! installed, registered, configured, executed, enabled, disabled, and removed
//! without modifying the core platform.
//!
//! # Submodules
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`commands`] | Tauri IPC commands exposed to the frontend |
//! | [`error`] | Domain error types and `CommandError` DTO |
//! | [`executor`] | Async subprocess supervision, timeout, and progress streaming |
//! | [`lifecycle`] | Plugin installation, removal, enable/disable, and state persistence |
//! | [`models`] | Data models, DTOs, and schema conforming types |
//! | [`payload`] | Execution payload assembly and pre-flight validation |
//! | [`registry`] | Plugin discovery, indexing, compatibility queries, and health checks |
//! | [`result_handler`] | Execution result parsing and run history recording |
//!
//! # Fault Isolation
//!
//! A failing plugin **must not** crash the core app. All subprocess operations
//! are async (Tokio) and the Tauri main thread is never blocked.

pub mod commands;
pub mod error;
pub mod executor;
pub mod lifecycle;
pub mod models;
pub mod payload;
pub mod registry;
pub mod result_handler;
