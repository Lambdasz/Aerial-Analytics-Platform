# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0-alpha] — 2026-10-28

Second functional milestone. Adds the plugin system, temporal change analysis, and a substantially more complete Module 1 (import, project management, DNG support, blur detection).

### Added

**Module 2 — Plugin Architecture & Extension Manager (full implementation)**
- ZIP-based plugin installation with Zip-Slip (CWE-22) and symlink-traversal (CWE-59) protections
- Plugin uninstall, enable/disable toggles, and per-plugin parameter overrides with atomic state persistence
- Runtime healthchecks (`<entrypoint> --healthcheck`) with automatic `.venv` detection
- Async plugin execution via Tokio with live progress streaming, process-tree abort on timeout or cancel, and sandboxed result delivery
- Plugin Manager UI: search, category/status filters, native ZIP install dialog, dynamic parameter forms generated from `parameters.json`, and an execution modal with abort control

**Module 10 — Simple Temporal Change Analysis**
- Eight analysis commands backed by real flight-session data: observation grouping by capture date, before/after period selection, vegetation change (ExG coverage), land-cover change detection, tree-count deltas, change-magnitude classification, significant-change-area detection, and change visualization

**Module 1 — Image Import & Project Management**
- Images are now **copied** into an app-owned library (`<app-data-dir>/library`, overridable via `AAP_LIBRARY_ROOT`) on import, so stored images survive the user moving or deleting the source folder
- Drop-zone import dialog with recursive folder import (up to 6 levels deep), chunked progress tracking, and resilient per-file batch handling
- Session image list with thumbnails, pagination, and per-image actions (delete, move to session, copy to session) — all async, non-blocking
- Project management UI: create, list, update, delete projects (atomic cascade deletion of child sessions and images)
- Metadata extractor panel showing live EXIF and XMP DJI telemetry read directly from image files
- DNG support: reads embedded JPEG previews without demosaic, serves image previews over IPC (`preview_image` command)
- Automatic blur detection (Laplacian-variance sharpness scoring) with session-level reclassification
- Flight session management with SQLite persistence (schema creation, migration, CRUD commands)

**Module 11 — Reporting Shell**
- App shell scaffold for the reporting and dashboard module

### Changed
- Module 1 EXIF parsing migrated from a hand-rolled TIFF/IFD parser to the `kamadak-exif` crate — absent or partially corrupt EXIF now composes into default tags instead of raising `MalformedExif`
- `AGENTS.md` synced with current codebase

### Security
- Plugin ZIP installation validates extraction paths against Zip-Slip (CWE-22) and rejects symlink traversal (CWE-59) before writing to disk
- Plugin uninstall blocks deletion of builtin plugins and rejects IDs not matching `^[a-z0-9-_]+$`
- Plugin execution sandboxes all file writes and result reads within per-job workspace boundaries

## [0.3.0-alpha] — 2026-09-23

First functional milestone. Core skeleton: Tauri 2 + React 19 + Blueprint.js shell, plugin discovery and lifecycle scaffolding, Module 1 data models, JSON Schema plugin contracts, and CI (lint, format, cross-platform build).

[0.4.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.3.0-alpha...v0.4.0-alpha
[0.3.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/releases/tag/v0.3.0-alpha
