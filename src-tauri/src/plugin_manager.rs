//! # Plugin System & Extension Manager (Module 2)
//!
//! Provides a plugin-based architecture so that analytical capabilities can be
//! installed, registered, configured, executed, enabled, disabled, and removed
//! without modifying the core platform.
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
//! │   Frontend   │─────▶│  commands.rs │─────▶│  payload.rs  │
//! │  (React/TS)  │◀─────│  (IPC cmds)  │◀─────│  (preflight) │
//! └──────────────┘      └──────┬───────┘      └──────────────┘
//!                              │
//!              ┌───────────────┼───────────────┐
//!              ▼               ▼               ▼
//!       ┌────────────┐  ┌────────────┐  ┌────────────┐
//!       │  registry  │  │  executor  │  │ lifecycle  │
//!       │ (discover) │  │  (spawn)   │  │ (install)  │
//!       └────────────┘  └────────────┘  └────────────┘
//!              │               │               │
//!              ▼               ▼               ▼
//!       ┌────────────┐  ┌────────────┐  ┌────────────┐
//!       │  models.rs │  │            │  │  error.rs  │
//!       │  (DTOs)    │  │            │  │  (types)   │
//!       └────────────┘  └────────────┘  └────────────┘
//! ```
//!
//! ## Submodules
//!
//! | Submodule | Responsibility |
//! |-----------|----------------|
//! | [`error`] | Domain error types and [`CommandError`](error::CommandError) DTO |
//! | [`executor`] | Imperative shell for async subprocess supervision, timeout enforcement, and progress streaming |
//! | [`executor_core`] | Pure core for execution DTOs, gating, payload construction, progress parsing, and result parsing |
//! | [`lifecycle`] | Imperative shell for plugin installation, removal, enable/disable, and state persistence |
//! | [`lifecycle_core`] | Pure core for plugin lifecycle validation, immutable state transitions, and removal planning |
//! | [`models`] | Pure data models, DTOs, and schema-conforming types |
//! | [`payload`] | Compatibility shim for Module 1 (`ImageMetadata`, preflight checks, and `payload.json` assembly) |
//! | [`registry`] | Imperative shell for plugin discovery, indexing, compatibility queries, and health checks |
//! | [`registry_core`] | Pure core for plugin registry state, compatibility predicates, and manifest/subcontract validation |
//! | [`result_handler`] | Compatibility shim for Module 9 (`ExecutionResult`, `result.json` parsing, and run history) |
//! | [`result_store`] | Impure SQLite persistence for per-image plugin metrics and artifact paths (`plugin_image_result`) |
//!
//! ## Purity Boundary
//!
//! Each domain separates a **Pure Core** (`*_core` modules and [`models`]) from an
//! **Imperative Shell** ([`registry`], [`lifecycle`], [`executor`], [`result_store`]):
//!
//! - **Pure Cores** (`registry_core`, `lifecycle_core`, `executor_core`) contain
//!   deterministic validation, state transitions, DTO transforms, and parallel
//!   parsing with zero I/O, clock, RNG, or Tauri state access.
//! - **Imperative Shells** (`registry`, `lifecycle`, `executor`, `result_store`)
//!   own filesystem I/O, ZIP extraction, async Tokio subprocesses, SQLite queries,
//!   and Tauri IPC / `RwLock` synchronization, delegating domain logic to their cores.
//!
//! ## Fault Isolation
//!
//! A failing plugin **must not** crash the core app. All subprocess operations
//! are async (Tokio) and the Tauri main thread is never blocked.
pub mod error;
pub mod executor;
pub mod executor_core;
pub mod lifecycle;
pub mod lifecycle_core;
pub mod models;
pub mod payload;
pub mod registry;
pub mod registry_core;
pub mod result_handler;
pub mod result_store;
