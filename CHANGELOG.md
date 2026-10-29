# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.0-alpha] — 2026-10-29

Sixth functional milestone. Completes Module 11: an analytics dashboard built from stored results, a printable report, and data exports.

### Added

- **Project dashboard**: sessions, images (with geotagged count), plots and total area, plugin result count, mean per indicator, images per plugin, and the latest temporal comparison — all read-only summaries of results other modules already stored
- **Interactive result explorer**: stored plugin metrics per image, with a detail dialog showing a lazily loaded image preview and a shortcut to open the image in the Map Explorer gallery
- **Plot comparison summary**: indicator means and per-plot values across the project, sharing the data behind the Module 9 ranking view
- **Temporal summary**: latest comparison and comparison history in the dashboard
- **Data export**: plot indicators as CSV, plots as GeoJSON, and a dashboard snapshot as JSON, all written to `map_storage/exports/` with the path reported in the UI
- **Printable report**: a self-contained HTML report per project (summary cards, inline SVG charts, plot indicators, plugin usage, temporal comparisons, result explorer), free of scripts and external assets so it can be archived or printed to PDF
- Cross-module navigation hook (`open-module`) so dashboard rows can jump to the map

### Changed

- The original Module 11 reporting contract is now a compiled compatibility layer: its `generate_report` and `export_data` commands delegate to the dashboard implementation instead of panicking; the report is HTML and exports are CSV/GeoJSON/JSON

### Removed

- The unused Module 11 stub tree (local duplicates of Module 9/10 types, `todo!()` aggregator/export/PDF scaffolding), superseded by `modules/module_11`

## [0.8.0-alpha] — 2026-10-29

Fifth functional milestone. Adds Module 9 (area & plot analytics) and Module 10 (temporal change analysis), and makes downstream analysis read plugin results instead of recomputing them.

### Added

**Module 9 — Area & Plot Analytics**

- Plot boundaries drawn on a map, imported from GeoJSON, or promoted from a saved Map Explorer area of interest, stored with geodesic area and perimeter
- Per-plot analysis: run any plugin once per plot with the boundary as its area of interest, storing the resulting indicators with units
- Plot statistics, multi-plot comparison ranked by any indicator, and a per-plot summary grouped by plugin
- A **Plots** tab inside the Map Explorer sidebar with the same workflow, plus a "N saved AOIs not added as plot(s)" prompt with one-click promotion
- Plots publishable as a map layer with area and perimeter in the popup

**Module 10 — Simple Temporal Change Analysis**

- Compare two sessions of a project: before/after imagery, vegetation coverage change in percentage points, interval in days, and magnitude classification
- Persisted comparison history per project, plus a JSON export containing the dashboard contract payload
- Spatial change detection: vegetation gain and loss regions between two dates, computed from masks, filtered by minimum area, and published as a map layer
- Explicit baseline/comparison session pickers replacing the implicit "latest two dates" behaviour

**Cross-module**

- Published plugin runs are stored per image (`plugin_image_result`) with metrics and artifact paths
- Module 10 prefers plugin coverage values and masks, falling back to its own excess-green measurement only when no run exists, and reports which source was used

### Changed

- Plugin results are persisted on publish, so in-app runs become readable by later analysis instead of being recomputed

### Fixed

- Restored the shared design tokens (colours, radii, shadows, fonts) that had disappeared from the global stylesheet, which had left map panels transparent and text unthemed
- Plugin Manager now uses the app's light theme instead of a hardcoded dark palette
- Ticking a plot checkbox no longer blanks the window (event field read after React cleared it)
- The analyse dialog now pre-selects a plugin and an analysis-ready image, so Run is enabled when the form looks complete
- Plots with no project (imported before a project was selected) stay visible under the project filter
- Map Explorer wraps its layout in an error boundary, showing a recoverable message instead of a white screen
- Cancelling an analysis can no longer affect other processes: stopping a run used a stale process id with a group-wide kill, which could terminate unrelated processes and log the user out. Cancellation is now bounded to the plugin's own process group and never reuses an id after a job ends

## [0.7.0-alpha] — 2026-10-29

Fourth functional milestone. Adds vegetation analysis Modules 4–6 as plugins, lets plugins be run and displayed straight from the app, and puts session imagery on the map.

### Added

**Vegetation plugins (Module 2 contract, Rust binaries)**

- Module 4 — RGB Vegetation Detection: vegetation masks from ExG/ExR/VARI with configurable thresholds, plus a tinted detection overlay and coverage/colour statistics
- Module 5 — Vegetation Coverage Analytics: coverage percentage and vegetated area from a canopy mask, low/medium/high classification, N×N zonation layer, and per-plot coverage with an average/highest/lowest comparison
- Module 6 — RGB Vegetation Condition Analysis: VARI green intensity, canopy RGB/HSL colour statistics, vigorous/moderate/severe condition classes, chlorosis (yellowing) and necrosis (browning) shares, condition zonation and per-plot comparison
- Plugins accept an optional explicit image extent so results can be placed on a known tile, and fall back to GPS plus altitude when an image carries no focal length

**Plugin execution from the app**

- Module 2 gains a Run Plugin action that picks project → session → image from the app database and launches the run with the plugin's parameter form
- Finished runs are published to Map Explorer automatically: GeoJSON artifacts become vector layers, georeferenced rasters become image overlays, and an open map updates without reloading
- Payload metadata (resolution, GPS, altitude, timestamp) is filled from the imported image when the caller does not supply it

**Map Explorer and Module 1**

- Session flight photos appear on the map as image overlays with popups and gallery cards, throttled thumbnail loading, and a per-photo visibility list under the Flight Photos layer entry
- The map centres on the newest session's imagery when it opens
- Imported images now store the camera heading from EXIF `GPSImgDirection`

**Tooling**

- `tools/connector`: read-only CLI over the app database (`projects`, `sessions`, `session`), a plugin runner with payload generation, timeout and result validation, and a publisher that writes run results into Map Explorer storage

### Changed

- Flight photo overlays keep their true estimated footprint instead of being shrunk to non-overlapping tiles; survey overlap is expected, and the outlines stay true to the camera coverage
- Flight photos render above the orthomosaic but below AOI, analysis results and annotations, so analysis layers stay clickable
- Plugin healthchecks re-run whenever the Plugin Manager opens, replacing stale statuses from earlier sessions

### Removed

- The demo mock plugin (`rgb-vegetation-exg`). It emitted a fixed Balikpapan polygon and was named identically to the real Module 4 plugin, so its results were mistaken for real analysis

### Fixed

- Plugin healthchecks and runs failed with `ENOENT` on every platform: a relative entrypoint was spawned with a relative working directory, so the program was resolved against the wrong folder. Plugin directories are now absolute at discovery and at both spawn sites
- Flight photo layers no longer disappear from the Layers panel when application state survives a hot reload
- Connector run outputs are ignored by git; the previous pattern was root-anchored and never matched `tools/connector/runs`

## [0.6.0-alpha] — 2026-10-29

Tooling milestone. Adds a containerized dev/CI toolchain; no application-code changes.

### Added

- `Dockerfile` for a headless dev/CI toolchain mirroring CI (Ubuntu 24.04, Node 24, Rust stable with clippy/rustfmt, Tauri Linux system deps), running as a non-root user with `HOST_UID`/`HOST_GID` overrides
- `compose.yaml` binding the repo into the toolchain container, with named volumes persisting cargo registry/git/target, npm cache, and `node_modules`
- `.dockerignore` to keep the image build context small

### Fixed

- `plugins/plugin_state.json` (app-generated runtime state) is now gitignored so it no longer breaks `npm run format:check` when present locally

## [0.5.0-alpha] — 2026-10-28

Third functional milestone. Ships the Module 3 Aerial Image Map Explorer, previously a placeholder backed by dummy markers.

### Added

**Module 3 — Aerial Image Map Explorer (full implementation)**

- Interactive Leaflet map with base-map switching, view controls, and drone image markers rotated by camera heading
- Orthomosaic import: GeoTIFF files are parsed, resampled to web-friendly tiles, and tracked by a background job queue with live progress; interrupted jobs are cleaned up on next launch
- Area of Interest (AOI): draw polygon boundaries on the map, save/rename/delete them, and reuse them as plugin analysis inputs
- Annotations: create, edit, and delete text/marker/shape notes pinned to the map
- Layer manager: toggle visibility, opacity, stacking order, and category filters for imagery, boundaries, and analysis results
- Spatial result viewer: renders Module 4/7/8 analysis outputs (land-cover, tree detection, vegetation) with popups, legends, and edit/delete flows
- Distance, area, and perimeter measurement tools
- Geotagged photo markers with import and gallery
- Map search bar and image gallery sidebar with two-way selection sync
- Sample map data (`map_storage/`, preview images) so the explorer has something to show out of the box
- `AAP_STORAGE_DIR` environment variable to relocate where map data is stored
- Dual-language (English & Indonesian) Module 3 architecture documentation under `docs/modules/module_3/`, plus a docs-completeness checker script

### Changed

- Module 3 source moved from `src/map/` and `src-tauri/src/map_controller/` to `src/modules/module-03/` and `src-tauri/src/modules/module_03/`; `map_controller` remains as a compatibility facade. Contributors importing from the old paths must update them.
- The Vite dev server now serves `/map_storage` so map tiles and imported imagery load during development

### Security

- The dev-server `/map_storage` route rejects path-traversal requests outside the storage root
- App instances take an exclusive lock (`map_storage/.app.lock`); a second instance runs read-only instead of racing the first for storage writes

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

[0.9.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.8.0-alpha...v0.9.0-alpha
[0.8.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.7.0-alpha...v0.8.0-alpha
[0.7.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.6.0-alpha...v0.7.0-alpha
[0.6.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.5.0-alpha...v0.6.0-alpha
[0.5.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.4.0-alpha...v0.5.0-alpha
[0.4.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/compare/v0.3.0-alpha...v0.4.0-alpha
[0.3.0-alpha]: https://github.com/Lambdasz/Aerial-Analytics-Platform/releases/tag/v0.3.0-alpha
