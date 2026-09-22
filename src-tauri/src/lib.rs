//! # Aerial Analytics Platform
//!
//! Desktop application for processing and analysing RGB aerial imagery
//! captured by standard drones. Built with [Tauri](https://tauri.app) and
//! React, the platform uses a **plugin-based architecture** so analytical
//! capabilities can be added independently without modifying the core system.
//!
//! ---
//!
//! ## Module 1 — Aerial Image & Project Manager
//!
//! Core system for organising aerial imagery, projects, acquisition sessions,
//! and associated metadata (EXIF, XMP DJI).
//!
//! - `models` — shared image metadata types ([`models::ImageMetadata`], [`models::ImageFormat`]).
//! - `modules::module_01` — image metadata extraction API
//!   (`modules::module_01::extract_metadata`).
//! - `commands::session` — flight session command stubs.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `create_session` | Create a new flight session |
//! | `get_session` | Fetch a session by id |
//! | `get_sessions_by_project` | List sessions for a project |
//! | `update_session_name` | Rename a session |
//! | `update_session_status` | Update a session's status |
//! | `assign_image_to_session` | Associate an image with a session |
//! | `recalculate_session_date_range` | Recompute a session's date range from its images |
//! | `delete_session` | Delete a session |
//!
//! ---
//!
//! ## Module 2 — Plugin System & Extension Manager
//!
//! Plugin architecture for installing, registering, configuring, executing,
//! enabling, disabling, and removing analytical plugins without modifying the
//! core platform.
//!
//! - `plugin_manager` — discovery, lifecycle, execution, and error isolation.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `validate_and_create_payload` | Pre-flight check and write `payload.json` |
//! | `process_execution_result` | Parse `result.json` and record run history |
//! | `start_plugin_job` | Launch an async plugin subprocess |
//! | `abort_plugin_job` | Kill a running plugin subprocess |
//! | `get_job_status` | Poll the lifecycle state of a job |
//! | `get_job_result` | Retrieve the full result of a completed job |
//!
//! **Status**: Models, error types, payload assembly, and result handling are
//! implemented. Registry discovery, lifecycle management, and async execution
//! are designed but contain `todo!()` stubs.
//!
//! ---
//!
//! ## Module 3 — Aerial Image Map Explorer
//!
//! Interactive geospatial environment for exploring aerial images, drawing
//! areas of interest, managing layers, and performing spatial measurements.
//!
//! ### Responsibilities
//!
//! - **Map Core & Image Location** — interactive Leaflet map, base-map
//!   switching, view controls, and drone image markers rotated by camera
//!   heading (`heading_deg`).
//! - **Map ↔ Image Interaction** — two-way synchronization between the map
//!   canvas and the image gallery sidebar via `ImageSyncContext`
//!   (single source of truth for `activeImageId` and interaction origin).
//! - **Plugin Execution & Result Handling Pipeline** — prepares plugin
//!   execution payloads (including AOI from this module), reads results back,
//!   and forwards spatial results to the visualization layer.
//! - **AOI, Geometry & Measurement** — pure geometry functions (coordinate
//!   validation, polygon validation, distance, area, perimeter, AOI creation).
//! - **Layer Management & Annotation** — layer visibility, opacity, stacking
//!   order, category filtering, and spatial annotation creation.
//! - **Spatial Result Integration** — bridges analytics module results
//!   (Modul 4/7/8) to the map frontend via the `SpatialResult` contract,
//!   emitted through Tauri events.
//!
//! ### Backend Structure (`map_controller`)
//!
//! - `map_controller` — backend controller for map operations
//!   (`map_controller::CommandError`, `map_controller::DroneImageMetadata`).
//! - `map_controller::error` — custom error types (`MapControllerError::code()`).
//! - `map_controller::types::sp_measurement_type` — spatial data contracts (`LayerPayload`, `SpatialResult`, `SpatialGeometry`).
//! - `map_controller::sp_measurement::geometry` — pure geometry functions for
//!   AOI & spatial measurement (no Leaflet dependency).
//! - `map_controller::layer` — layer management & annotation pure functions.
//! - `map_controller::sp_measurement::spatial_rs` — spatial result integration
//!   with Modul 4/7/8 via Tauri events (`map_controller::MapLayerCommand`,
//!   `LayerPayload`, `SpatialResult`).
//! - `map_controller::sp_measurement::validation_input` — validation logic
//!   (`SpatialGeometry::validate()`, `SpatialResult::validate()`, `LayerPayload::validate()`).
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `get_project_image_markers` | Generate pseudo-random drone image markers for the map explorer |
//! | `validate_and_create_payload` | Pre-flight check and write `payload.json` |
//! | `process_execution_result` | Parse `result.json` and record run history |
//! | `get_dummy_spatial_layers` | Load dummy spatial layers (dev/testing only) |
//!
//! ### Key Functions
//!
//! #### Map Core
//!
//! - `get_project_image_markers(count)` → generates `count` markers scattered
//!   around the base coordinate (`base_lat: -1.247`, `base_lng: 116.893`).
//!   Pseudo-random data keeps this module light (no `rand` crate); invoking
//!   with the same `count` produces consistent results per index sequence.
//!
//! #### Plugin Execution & Result Handling Pipeline
//!
//! - `preflight_check(image_meta, inputs_req)` → fail-fast validation that
//!   image metadata satisfies plugin input requirements (GPS presence,
//!   allowed formats) before payload assembly.
//! - `assemble_payload(payload, output_dir)` → writes the `payload.json`
//!   "order letter" (pretty-printed `ExecutionPayload`) that the Python
//!   plugin process reads on execution.
//! - `validate_and_create_payload(payload, inputs_req, output_dir)` → first
//!   gate before plugin execution; validates input then assembles
//!   `payload.json` in the output directory.
//! - `parse_execution_result(output_dir)` → bridges raw Python plugin output
//!   (`result.json`) into an `ExecutionResult` struct; errors when the file
//!   is missing, unreadable, or status is `"error"`.
//! - `record_run_history(history_file_path, entry)` → appends an audit-trail
//!   entry (timestamp, plugin_id, status, output_dir, output_files) to
//!   `history.json`; resets to an empty array without crashing if the file
//!   is corrupt.
//! - `process_execution_result(output_dir, history_file_path, plugin_id)` →
//!   reads `result.json` and records the run to history; `output_files` here
//!   can later be mapped into `SpatialResult` for rendering.
//!
//! #### Geometry & AOI (pure functions)
//!
//! - `validate_coordinate(longitude, latitude)` → valid if longitude within
//!   `-180..180` and latitude within `-90..90`.
//! - `validate_polygon(polygon)` → valid if ring closed, ≥3 unique points,
//!   not self-intersecting, and all coordinates valid.
//! - `calculate_distance(point_a, point_b)` → distance in meters.
//! - `calculate_area(polygon)` → area in m² (WGS84).
//! - `calculate_perimeter(polygon)` → total perimeter in meters.
//! - `create_aoi(aoi_id, name, polygon, created_at, area_sq_m, perimeter_m)`
//!   → builds a standard `AoiFeature` (GeoJSON `Feature<Polygon>`) — the
//!   spatial contract forwarded to Modul 2 and other analytics modules.
//!
//! #### Layer Management & Annotation (pure functions)
//!
//! - `toggle_layer_visibility(layer_id, is_visible)` → toggle a layer on/off
//!   without touching the DOM.
//! - `set_layer_opacity(layer_id, opacity)` → set transparency, must be
//!   within `0.0 ≤ opacity ≤ 1.0`.
//! - `reorder_layer_stack(layers, from_index, to_index)` → restack layers
//!   (Z-index) without mutating the input array.
//! - `filter_layers_by_category(layers, category)` → subset layers matching
//!   a category (e.g. "Satellite", "Analysis").
//! - `create_annotation(annotation_id, text, geometry, created_at)` → builds
//!   a GeoJSON `Feature` annotation (text note, point marker, or shape).
//!
//! #### Spatial Result Integration
//!
//! - `dispatch_spatial_result(app_handle, layer_payload)` → main entry point
//!   of inter-module communication: validates the payload, then emits the
//!   `map://spatial-result` Tauri event so the Leaflet frontend renders the
//!   layer (replacing existing `layer_id` if present).
//! - `dummy::get_dummy_spatial_layers()` → parses embedded
//!   `dummy_sp_result.json` (3 dummy layers: tree detection, vegetation
//!   index, building detection) so the UI can be built in parallel with
//!   Modul 4/7/8. **Dev/testing only — must not be used in production flow.**
//! - `SpatialGeometry::validate()` → GeoJSON-level geometry integrity checks
//!   (closed rings, ≥4 points, exactly 2 coordinates per point).
//! - `SpatialResult::validate()` → delegates to `SpatialGeometry::validate()`;
//!   extension point for future property validation (e.g. confidence 0–1).
//! - `LayerPayload::validate()` → highest-level fail-fast gate: ImageOverlay
//!   opacity within 0.0–1.0 AND all `spatial_results` valid, before emitting.
//! - `MapControllerError::code()` → maps error variants to stable string
//!   codes (`"INVALID_GEOMETRY"`, `"IO_ERROR"`) so the TypeScript frontend
//!   can `switch (error.code)` reliably.
//!
//! ### Frontend (`src/map/`)
//!
//! - `MapCanvas` — main map container (base `TileLayer`, clustered markers,
//!   basemap state switching).
//! - `MapViewControlBar` — zoom in/out, fit-to-bounds, reset view, fullscreen
//!   (Native Fullscreen API).
//! - `DroneImageMarker` — custom SVG marker (drone/arrow icon) auto-rotated by
//!   `heading_deg`; click updates global sync state.
//! - `ImagePopupCard` — glassmorphism popup showing telemetry (lat, lon,
//!   altitude, heading) + thumbnail.
//! - `MapSyncHandler` — invisible component; flies the camera (`flyTo`) to the
//!   image clicked in the gallery (gallery → map flow).
//! - `ImageGallerySidebar` — image list; auto-scrolls (`scrollIntoView`) to
//!   the item whose marker was clicked on the map (map → gallery flow).
//! - `ImageSyncProvider` / `useImageSync` — Context API state management,
//!   single source of truth for `activeImageId` and interaction origin.
//! - `invokeMap` — generic helper for invoking Tauri commands with error
//!   logging and re-throw.
//!
//! ### Data Contracts
//!
//! - **Spatial Result Contract** (Modul 3 ↔ 4/7/8): `MapLayerCommand`
//!   (`action: "load_spatial_result"` + `LayerPayload`) with GeoJSON-style
//!   geometry (`[lon, lat]`) and per-type properties (Tree / Vegetation /
//!   Building).
//! - **Plugin Execution Contract** (Modul 2 ↔ Python plugin):
//!   `ExecutionPayload` → `payload.json`; `ExecutionResult` ← `result.json`;
//!   `RunHistoryEntry` → `history.json`.
//!
//! ### Data Contracts Reference
//!
//! **1. Spatial Result Contract (Modul 3 ↔ Modul 4/7/8)**
//! | Tipe Data | Field | Tipe | Keterangan |
//! |---|---|---|---|
//! | `MapLayerCommand` | `action` | `String` | Instruksi frontend (`"load_spatial_result"`) |
//! | `MapLayerCommand` | `payload` | `LayerPayload` | Payload konfigurasi + data spasial |
//! | `LayerPayload` | `layer_id`/`name` | `String` | ID & nama layer |
//! | `LayerPayload` | `display_preference`| `LayerDisplayMode` | `ImageOverlay` / `Point` |
//! | `LayerPayload` | `spatial_results` | `Vec<SpatialResult>` | Kumpulan fitur spasial |
//! | `SpatialResult` | `source`, `geometry`, `properties` | `String`, `SpatialGeometry`, `SpatialProperties` | Detail fitur analitik |
//!
//! **2. Plugin Execution Contract (Modul 2 ↔ Plugin Python)**
//! | Tipe Data | Field | Tipe | Keterangan |
//! |---|---|---|---|
//! | `ExecutionPayload` | `plugin_id`, `image`, `aoi`, dll. | `String`, `ImageMetadata`, dll. | Ditulis ke `payload.json` |
//! | `ExecutionResult` | `status`, `metrics`, `output_files` | `String`, `Map`, `Vec<String>` | Dibaca dari `result.json` |
//!
//! ### Function Input/Output Reference
//!
//! | Bagian | Fungsi / Komponen | Input | Output |
//! |---|---|---|---|
//! | **Backend** | `get_project_image_markers` | `count: usize` | `Ok(Vec<DroneImageMetadata>)` |
//! | **Frontend**| `useImageMarkers` (Hook) | `count: number` | `{ markers, isLoading }` |
//! | **Frontend**| `MapCanvas`, `MapViewControlBar`, dll. | `{}` (Props kosong) | `JSX Element` |
//! | **Backend** | `validate_and_create_payload` | `payload, inputs_req, output_dir` | `Ok(String)` |
//! | **Backend** | `process_execution_result` | `output_dir, history, plugin_id` | `Ok(ExecutionResult)` |
//! | **Backend** | `preflight_check` | `&ImageMetadata`, `&InputsRequirement`| `Ok(())` |
//! | **Backend** | `assemble_payload` | `&ExecutionPayload`, `&Path` | `Ok(PathBuf)` |
//! | **Backend** | `parse_execution_result` | `&Path` | `Ok(ExecutionResult)` |
//! | **Backend** | `record_run_history` | `&Path`, `RunHistoryEntry` | `Ok(())` |
//! | **Backend** | `validate_coordinate` | `lon: f64`, `lat: f64` | `Result<(), GeometryError>` |
//! | **Backend** | `validate_polygon` | `polygon: Polygon` | `Result<(), GeometryError>` |
//! | **Backend** | `calculate_distance` | `Coordinate`, `Coordinate` | `distance_m: f64` |
//! | **Backend** | `calculate_area` | `polygon: Polygon` | `area_sq_m: f64` |
//! | **Backend** | `calculate_perimeter` | `polygon: Polygon` | `perimeter_m: f64` |
//! | **Backend** | `create_aoi` | `id, name, poly, date, area, peri` | `AoiFeature` |
//! | **Backend** | `toggle_layer_visibility` | `layer_id: String`, `is_visible: bool`| `Result<LayerState, LayerError>` |
//! | **Backend** | `set_layer_opacity` | `layer_id: String`, `opacity: f64` | `Result<LayerState, LayerError>` |
//! | **Backend** | `reorder_layer_stack` | `layers, from_index, to_index` | `Result<Array, LayerError>`|
//! | **Backend** | `create_annotation` | `id, text, geometry, date` | `AnnotationFeature` |
//! | **Backend** | `filter_layers_by_category`| `layers`, `category: String` | `Array of Layer` |
//! | **Backend** | `dispatch_spatial_result` | `&AppHandle`, `LayerPayload` | `Ok(())` atau `Err` |
//! | **Backend** | `dummy::get_dummy_spatial_layers`| `()` | `Ok(Vec<MapLayerCommand>)` |
//! | **Backend** | `SpatialGeometry::validate` | `&self` | `Ok(())` atau `Err` |
//! | **Backend** | `SpatialResult::validate` | `&self` | `Ok(())` atau `Err` |
//! | **Backend** | `LayerPayload::validate` | `&self` | `Ok(())` atau `Err` |
//! | **Backend** | `MapControllerError::code` | `&self` | `String` |
//!
//!
//! **Status**: Implemented. Map core, image markers, geometry/AOI, layer
//! management, annotation, and spatial result integration are complete. Dummy
//! spatial layers available for frontend development in parallel with
//! Modul 4/7/8.
//!
//! ---
//!
//! ## Module 4 — RGB Vegetation Detection Plugin
//!
//! Plugin for identifying vegetation from standard RGB aerial imagery using
//! indices such as ExG, ExR, or VARI. Generates vegetation masks and
//! statistics.
//!
//! **Status**: Python plugin skeleton at `plugins/rgb-vegetation-detection/`
//! (simulated detection). No Rust backend code.
//!
//! ---
//!
//! ## Module 5 — Vegetation Coverage Analytics
//!
//! Quantifies vegetation coverage using outputs from Module 4. Calculates
//! coverage percentages, classifies areas (low / medium / high), and supports
//! plot-based and multi-plot comparison.
//!
//! **Status**: Not yet implemented.
//!
//! ---
//!
//! ## Module 6 — RGB Vegetation Condition Analysis
//!
//! Analyses visible vegetation conditions using RGB colour information:
//! greenness intensity, yellowing, browning, and condition zonation.
//!
//! **Status**: Not yet implemented.
//!
//! ---
//!
//! ## Module 7 — Tree Detection & Counting Plugin
//!
//! Computer-vision plugin for detecting and counting individual trees or tree
//! crowns from aerial RGB imagery. Returns spatial locations, confidence
//! values, and density metrics.
//!
//! **Status**: Mock Rust plugin at `plugins/mock_rust/` (tree-canopy-density).
//! No real CV model integrated.
//!
//! ---
//!
//! ## Module 8 — RGB Land-Cover Classification Plugin
//!
//! Plugin for classifying visible land-cover types (vegetation, bare soil,
//! water, built areas) from RGB aerial imagery. Produces classified masks,
//! per-class statistics, and boundary vectors.
//!
//! **Status**: Python plugin at `plugins/rgb-landcover-classification/` with
//! Random Forest classifier. Model weights not yet trained.
//!
//! ---
//!
//! ## Module 9 — Area & Plot Analytics
//!
//! Spatial analytical system for defining plots and summarising analytical
//! results within them. Aggregates per-plot statistics for the dashboard.
//!
//! - `plot` — GeoJSON plot import and validation (`plot::parse_plots`).
//! - `plot_api` — cross-module API for plot definition, per-plot analysis,
//!   and statistics ([`plot_api::define_plot`], [`plot_api::analyze_plot`],
//!   [`plot_api::get_plot_statistics`]).
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `import_plots` | Parse a GeoJSON file into plot boundaries |
//!
//! **Status**: GeoJSON parser fully implemented with unit tests. Plot API
//! functions are `todo!()` stubs.
//!
//! ---
//!
//! ## Module 10 — Simple Temporal Change Analysis
//!
//! Compares RGB aerial observations captured at different times. Calculates
//! changes in vegetation coverage, land-cover transitions, and tree-count
//! deltas between observation periods.
//!
//! - `modules::module_10` — cross-module data contracts for change area
//!   detection and temporal results.
//!
//! **Status**: Cross-module contract types and constructors implemented with
//! unit tests. Multi-date management and comparison logic not yet implemented.
//!
//! ---
//!
//! ## Module 11 — Aerial Analytics Dashboard & Reporting
//!
//! Integrated interface for summarising and communicating results: project
//! dashboards, plot comparison, temporal summaries, chart generation, report
//! export, and GIS data export.
//!
//! **Status**: Not yet implemented.

mod commands;
mod map_controller;
mod modules;
mod plot;
mod plugin_manager;

use crate::map_controller::sp_measurement::spatial_rs::dummy::get_dummy_spatial_layers;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use plugin_manager::executor::ActiveJobTracker;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared application state injected into every Tauri command via `.manage()`.
pub struct AppState {
    /// Active and recently completed plugin jobs (Role 1 — executor.rs).
    pub jobs: Arc<RwLock<ActiveJobTracker>>,
}

// Learn more about Tauri commands at
// https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn import_plots(path: String) -> Result<plot::ImportResult, String> {
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| format!("Failed to read file '{path}': {e}"))?;

    plot::parse_plots(&text)
}

pub mod models;

pub mod plot_api;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // Role 3 — Data Bridge
            plugin_manager::commands::validate_and_create_payload,
            plugin_manager::commands::process_execution_result,
            // Role 1 — Subprocess Supervisor & Isolation Engineer
            plugin_manager::executor::start_plugin_job,
            plugin_manager::executor::abort_plugin_job,
            plugin_manager::executor::get_job_status,
            plugin_manager::executor::get_job_result,
            // Plot import
            import_plots,
            get_dummy_spatial_layers,
            // Flight session
            commands::session::create_session,
            commands::session::get_session,
            commands::session::get_sessions_by_project,
            commands::session::update_session_name,
            commands::session::update_session_status,
            commands::session::assign_image_to_session,
            commands::session::recalculate_session_date_range,
            commands::session::delete_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
