//! # Aerial Analytics Platform
//!
//! Desktop application for processing and analysing RGB aerial imagery
//! captured by standard drones. Built with [Tauri](https://tauri.app) and
//! Blueprint.js, the platform uses a **plugin-based architecture** so analytical
//! capabilities can be added independently without modifying the core system.
//!
//! ---
//!
//! ## Project Brief
//!
//! ### Lecturer's Main Task
//!
//! - Ensure that the project and its outcomes are verifiable.
//! - Ensure that functional thinking has been properly implemented throughout the development process.
//! - Ensure that the project is ready for open source development.
//!
//! ### Technology
//!
//! Blueprint.js (for interface and light computation) + Tauri + Python (if necessary for AI).
//!
//! ### Planned Features per Module
//!
//! The *possible features* below are the original brief's scope. Implementation
//! status lives in each `## Module N` section further down.
//!
//! #### Module 1 — Aerial Image & Project Manager
//!
//! Develop the core system for organizing aerial imagery, projects, acquisition sessions, and associated metadata.
//!
//! **Possible Features:**
//!
//! - **Project Management**: Create, update, and organize aerial monitoring projects.
//! - **RGB Image Import**: Import and organize aerial images captured using standard drones.
//! - **Image Metadata Extraction**: Extract available metadata such as GPS coordinates, acquisition date, altitude, and image resolution.
//! - **Flight Session Management**: Organize images according to flight or acquisition sessions.
//! - **Image Quality Checking**: Identify images with blur, poor exposure, or missing metadata.
//! - **Dataset Summary**: Provide statistics about images and acquisition sessions.
//!
//! #### Module 2 — Plugin System & Extension Manager
//!
//! Develop a plugin architecture that allows analytical capabilities to be installed, registered, configured, executed, enabled, disabled, and removed without modifying the core platform.
//!
//! **Possible Features:**
//!
//! - **Plugin Interface Definition**: Define a standard interface that all analytical plugins must follow.
//! - **Plugin Registration**: Register plugin name, version, description, supported input, and output.
//! - **Plugin Discovery**: Automatically detect available plugins.
//! - **Plugin Installation**: Allow users to add new plugins to the platform.
//! - **Plugin Enable and Disable**: Activate or deactivate plugins without deleting them.
//! - **Plugin Configuration**: Allow each plugin to expose configurable parameters.
//! - **Plugin Execution**: Execute plugins using standardized image, project, or area-of-interest inputs.
//! - **Plugin Result Handling**: Receive and standardize plugin outputs.
//! - **Plugin Error Isolation**: Prevent a failed plugin from crashing the core platform.
//! - **Plugin Information Viewer**: Display installed plugins and their capabilities.
//!
//! #### Module 3 — Aerial Image Map Explorer
//!
//! Develop an interactive geospatial environment for exploring aerial images and their associated locations.
//!
//! **Possible Features:**
//!
//! - **Geo-Referenced Image Explorer**: Display aerial images according to their GPS locations.
//! - **Image Location Marker**: Show where each image was captured.
//! - **Area of Interest Selection**: Allow users to define specific areas for analysis.
//! - **Layer Management**: Display imagery, boundaries, annotations, and analytical results.
//! - **Spatial Measurement**: Measure distance, area, and perimeter.
//! - **Spatial Annotation**: Mark and annotate locations or areas of interest.
//!
//! #### Module 4 — RGB Vegetation Detection Plugin
//!
//! Develop a plugin for identifying vegetation from standard RGB aerial imagery.
//!
//! **Possible Features:**
//!
//! - **Vegetation Detection**: Separate vegetation from non-vegetation areas.
//! - **RGB Vegetation Indices**: Apply RGB-based indices such as ExG, ExR, or VARI.
//! - **Vegetation Mask Generation**: Generate vegetation masks from aerial imagery.
//! - **Green Pixel Analysis**: Analyze vegetation based on RGB color characteristics.
//! - **Detection Threshold Configuration**: Allow users to adjust detection parameters.
//! - **Detection Visualization**: Generate results that can be displayed by the platform.
//! - **Plugin-Compatible Output**: Return standardized masks, statistics, and metadata.
//!
//! #### Module 5 — Vegetation Coverage Analytics
//!
//! Develop a system for quantifying vegetation coverage using outputs from the RGB Vegetation Detection Plugin.
//!
//! **Possible Features:**
//!
//! - **Vegetation Coverage Estimation**: Calculate the percentage of an area covered by vegetation.
//! - **Vegetation Area Calculation**: Estimate total vegetation-covered area.
//! - **Coverage Classification**: Categorize areas into low, medium, or high vegetation coverage.
//! - **Coverage Zonation**: Divide study areas according to vegetation coverage.
//! - **Plot-Based Coverage Analysis**: Calculate vegetation coverage for individual plots.
//! - **Multi-Plot Comparison**: Compare vegetation coverage among selected plots.
//! - **Coverage Visualization**: Display vegetation coverage statistics and spatial results.
//!
//! #### Module 6 — RGB Vegetation Condition Analysis
//!
//! Develop a system for analyzing visible vegetation conditions using RGB color information.
//!
//! **Possible Features:**
//!
//! - **Green Intensity Analysis**: Analyze differences in vegetation greenness.
//! - **Vegetation Color Analysis**: Analyze RGB color characteristics of detected vegetation.
//! - **Vegetation Condition Classification**: Categorize vegetation into visual condition classes.
//! - **Yellowing Detection**: Identify vegetation areas showing visible yellowing.
//! - **Browning Detection**: Identify vegetation areas showing browning or dry appearance.
//! - **Condition Zonation**: Divide areas according to visually derived vegetation conditions.
//! - **Plot Condition Comparison**: Compare vegetation conditions among selected plots.
//!
//! #### Module 7 — Tree Detection & Counting Plugin
//!
//! Develop a computer vision plugin for detecting and counting visible trees from aerial RGB imagery.
//!
//! **Possible Features:**
//!
//! - **Tree Detection**: Detect individual trees or tree crowns.
//! - **Tree Counting**: Calculate the total number of detected trees.
//! - **Confidence Filtering**: Filter detections according to prediction confidence.
//! - **Tree Location Mapping**: Return spatial locations of detected trees.
//! - **Area-Based Tree Counting**: Count trees within selected plots.
//! - **Tree Density Analysis**: Calculate tree density per unit area.
//! - **Detection Review**: Allow users to inspect and correct detections.
//! - **Plugin-Compatible Output**: Return standardized detections, counts, coordinates, and confidence values.
//!
//! #### Module 8 — RGB Land-Cover Classification Plugin
//!
//! Develop a plugin for classifying visible land-cover types from standard RGB aerial imagery.
//!
//! **Possible Features:**
//!
//! - **Land-Cover Classification**: Classify visible areas into vegetation, bare soil, water, built areas, or other relevant classes.
//! - **Custom Class Definition**: Allow projects to define suitable land-cover categories.
//! - **Land-Cover Mask Generation**: Generate classified masks from aerial images.
//! - **Class Area Calculation**: Calculate the area occupied by each land-cover class.
//! - **Land-Cover Percentage**: Calculate the proportion of each class within a selected area.
//! - **Plot-Based Land-Cover Analysis**: Compare land-cover composition among selected plots.
//! - **Land-Cover Map Generation**: Produce classification results that can be displayed on the platform.
//! - **Plugin-Compatible Output**: Return standardized land-cover classes, masks, statistics, and metadata.
//!
//! #### Module 9 — Area & Plot Analytics
//!
//! Develop a spatial analytical system for defining plots and summarizing analytical results within them.
//!
//! **Possible Features:**
//!
//! - **Plot Definition**: Create or import plot boundaries.
//! - **Area Calculation**: Calculate plot area and perimeter.
//! - **Analysis by Plot**: Aggregate analytical results within individual plot boundaries.
//! - **Plot Statistics**: Calculate summary statistics for each plot.
//! - **Multi-Plot Comparison**: Compare indicators among different plots.
//! - **Plot Ranking**: Rank plots according to selected analytical indicators.
//! - **Plot Summary**: Generate a structured analytical summary for each plot.
//!
//! #### Module 10 — Simple Temporal Change Analysis
//!
//! Develop a system for comparing RGB aerial observations captured at different times.
//!
//! **Possible Features:**
//!
//! - **Multi-Date Image Management**: Organize aerial observations according to acquisition date.
//! - **Before-and-After Comparison**: Compare imagery from two observation periods.
//! - **Vegetation Coverage Change**: Calculate changes in vegetation coverage.
//! - **Land-Cover Change**: Identify visible land-cover changes.
//! - **Tree Count Change**: Compare detected tree counts between observation periods.
//! - **Change Magnitude Analysis**: Calculate the amount of observed change.
//! - **Change Area Detection**: Identify locations where significant change has occurred.
//! - **Change Visualization**: Display before-and-after imagery and analytical results.
//!
//! #### Module 11 — Aerial Analytics Dashboard & Reporting
//!
//! Develop an integrated interface for summarizing and communicating results produced by the platform and installed plugins.
//!
//! **Possible Features:**
//!
//! - **Project Analytics Dashboard**: Display important indicators for each project.
//! - **Plugin Result Dashboard**: Display outputs generated by installed analytical plugins.
//! - **Analysis Result Summary**: Summarize results from vegetation, tree, land-cover, plot, and temporal modules.
//! - **Plot Comparison Dashboard**: Compare analytical indicators among selected plots.
//! - **Temporal Summary**: Present changes across observation periods.
//! - **Interactive Result Explorer**: Connect analytical summaries with corresponding images and locations.
//! - **Charts and Statistics**: Present analytical results using charts and summary statistics.
//! - **Report Generation**: Generate structured reports containing maps, figures, and analytical results.
//! - **Data Export**: Export results for GIS, statistical, or further research analysis.
//!
//! ---
//!
//! ## Development Priorities
//!
//! Modules are prioritised by their role in the upcoming presentation and
//! the demo itself:
//!
//! | Priority | Modules | Rationale |
//! |----------|---------|-----------|
//! | **Critical** | 1, 2, 3 | The application does not run without these. |
//! | **High** | 4 + 5 + 6 (single plugin), 7, 8 | Proves the plugin architecture — without them the demo is just a map. Implementation details still subject to paper review. |
//! | **Medium** | 9, 10 | Simplify plugin usage and analysis; the app works without them, but with more friction. |
//! | **Low** | 11 | Only fully functional once all other modules are complete. |
//!
//! ---
//!
//! ## Module 1 — Aerial Image & Project Manager
//!
//! Core system for organising aerial imagery, projects, acquisition sessions,
//! and associated metadata (EXIF, XMP DJI).
//!
//! ```text
//! Project
//! ├── Session 1
//! │   ├── Image 1
//! │   ├── Image 2
//! │   ├── ...
//! │   └── Image n
//! └── Session 2
//!     ├── Image 1
//!     ├── Image 2
//!     ├── ...
//!     └── Image n
//! ```
//!
//! - `models::image` — shared image types ([`models::ImageMetadata`], [`models::ImageFormat`],
//!   plus the import request/report types).
//! - `modules::module_01::metadata_extractor` — EXIF/XMP DJI metadata extraction
//!   (`extract_metadata`): camera/GPS tags, acquisition timestamp, pixel
//!   dimensions, format sniffing, completeness checks, and the plugin
//!   pre-flight projection.
//! - `modules::module_01::dng_preview` — the JPEG a DNG already stores
//!   (`read_dng_preview`), so a browser can display a DNG with no raw decoder.
//! - `modules::module_01::image_importer` — RGB import planning (`import_images`):
//!   per-file classification, filesystem-aware name-conflict detection
//!   (report or auto-suffix), project-scope duplicate detection, and
//!   incomplete-metadata flags.
//! - `modules::module_01::import_io` — copy-on-import I/O (`run_import`,
//!   `analysis_input`): copies accepted images into `<library_root>`, reads
//!   metadata from the copy, writes the row(s), links JPG/DNG stem pairs, and
//!   removes copies when their session is deleted or a startup sweep finds
//!   them orphaned. Folder imports recursively scan nested subdirectories up
//!   to 6 levels deep (skipping hidden folders and symlinked directories while
//!   supporting symlinked image files).
//! - `modules::module_01::flight_session` — flight session CRUD and the SQLite
//!   `session`/`image` schema, including the crash-safe `image` table migration.
//! - `modules::module_01::blur_detection` — Laplacian-variance sharpness
//!   scoring (`compute_blur_score`) on a grayscale buffer downscaled to a
//!   1280px long edge when the source is larger — never upscaled, since that
//!   would smooth away the edges the Laplacian measures and score a small
//!   source as falsely blurry, so a source already below the cap (a DNG's
//!   embedded preview, or a small JPEG) is left unscored instead. JPEG
//!   decoded directly, DNG from its stored preview. `blur_score` is computed
//!   per image during `import_io::run_import`; `is_blurry` is then
//!   re-derived for every scored image in the session in one pass
//!   (`flight_session::reclassify_session_blur`, called once per import run)
//!   from the session's mean/stddev z-score, falling back to an absolute
//!   hard floor only while the session has too few scored images for a
//!   baseline — not frozen per image at insert time, so the flag depends
//!   only on the
//!   session's current score set, not on import order or batch boundaries.
//! - `commands::session` — flight session Tauri commands.
//! - `commands::import` — copy-on-import commands (`import_images`,
//!   `get_analysis_input`) and the `import-progress` event.
//! - `commands::image_preview` — serves a `.jpg`/`.jpeg`/`.dng` as a
//!   base64-encoded JPEG preview over IPC.
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `import_images` | Copy picked JPG/DNG files into the app library and record them in a session |
//! | `get_analysis_input` | Return the image a plugin may analyse (JPEG, or a DNG's same-stem JPEG) |
//! | `preview_image` | Return a displayable JPEG preview of an aerial image (JPEG as-is or downscaled, DNG from its stored preview) |
//! | `thumbnail_image` | Return a small cached thumbnail of an aerial image (its EXIF thumbnail when present, otherwise a 160 px resize) |
//! | `create_session` | Create a new flight session |
//! | `get_session` | Fetch a session by id |
//! | `get_sessions_by_project` | List sessions for a project |
//! | `update_session_name` | Rename a session |
//! | `update_session_status` | Update a session's status |
//! | `assign_image_to_session` | Associate an image with a session |
//! | `recalculate_session_date_range` | Recompute a session's date range from its images |
//! | `delete_session` | Delete a session |
//! | `delete_image` | Delete one image and move its library copy to the trash (requires `confirm`) |
//! | `move_image_to_session` | Move one image (row and file) into another session of the same project |
//! | `copy_image_to_session` | Copy one image (row and file) into another session of the same project |
//! | `get_image_metadata` | Read EXIF and XMP from a stored image's file (exposure and DJI tags the row does not keep) |
//!
//! **Status**: Flight session commands, metadata extraction (JPEG and DNG),
//! import planning, copy-on-import I/O and the schema migration are
//! implemented and unit tested. `import_images` copies picked images into the
//! app-owned library (`<app-data-dir>/library`, override `AAP_LIBRARY_ROOT`)
//! and stores the copy's absolute path, so the source folder may later be
//! moved or deleted. `get_analysis_input` resolves the row a plugin may
//! analyse; a DNG with no same-stem JPEG is imported but reported as not
//! analysis-ready, because Module 1 does not decode raw pixels.
//! `read_dng` is the crate's entry point for reading a DNG and
//! `read_dng_preview` for reading its stored JPEG preview, so no module has
//! to reach into Module 1's TIFF parser; no other module calls them yet.
//! The `preview_image` command serves those previews over IPC.
//! Blur detection (M1-29) is also implemented: every imported image gets a
//! `blur_score`/`is_blurry` pair persisted alongside it, computed and
//! classified at import time (no new command — it is not yet exposed over
//! IPC beyond the `Image` record's own fields). Known, deliberate
//! limitations: DNGs are scored from their camera-processed preview, not
//! raw sensor data; no sensor-noise pre-filtering is applied; scoring is
//! sequential, not parallelised. Surfacing quality flags in the UI (M1-32)
//! is a separate, not-yet-implemented task.
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
//! - `map_controller` — backend controller for map operations; see the
//!   [`map_controller`] module docs for
//!   responsibilities, key functions, data contracts, and the full I/O
//!   reference.
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
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`).
//! No Rust backend code — the platform side is entirely Module 2.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/rgb-vegetation-detection/main.py` — `def main() -> None` (Python) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//!
//! Entrypoint (`main.py`, Python):
//!
//! ```python
//! def main() -> None:
//!     # --healthcheck
//!     # --input <execution_payload.json> --output <execution_result.json>
//! ```
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
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`) —
//! identical contract, runtime type is `binary` instead of `python`.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/mock_rust/src/main.rs` — `fn main()` (Rust binary) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//! | Runtime | `binary` (`manifest.json` → `runtime.entrypoint = "bin/mock_rust"`) |
//!
//! Entrypoint (`src/main.rs`, Rust binary):
//!
//! ```rust,ignore
//! fn main() {
//!     // --healthcheck
//!     // --input <payload.json> --output <result.json>
//! }
//! ```
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
//! ### Plugin Contract
//!
//! Uses the Module 2 plugin template architecture (`plugins/template/`).
//! No Rust backend code — the platform side is entirely Module 2.
//!
//! | Item | Value |
//! |------|-------|
//! | Architecture | Module 2 plugin template (`plugins/template/`) |
//! | Interface | `manifest.json`, `parameters.json`, `inputs.json`, `outputs.json` |
//! | Entrypoint | `plugins/rgb-landcover-classification/main.py` — `def main() -> None` (Python) |
//! | CLI | `--healthcheck` \| `--input <payload.json> --output <result.json>` |
//! | Execution | isolated subprocess via Module 2 supervisor: `payload.json` in → `result.json` out |
//! | Progress | stdout `PROGRESS: {json}` lines parsed by Module 2 |
//!
//! Entrypoint (`main.py`, Python):
//!
//! ```python
//! def main() -> None:
//!     # --healthcheck
//!     # --input <payload.json> --output <result.json>
//! ```
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
//!   detection and temporal results (`modules::module_10::functions`).
//!
//! ### Tauri Commands
//!
//! | Command | Description |
//! |---------|-------------|
//! | `get_observation_periods` | Group a session's images per acquisition date (10.1) |
//! | `get_vegetation_change` | Compare vegetation coverage of the two latest periods (10.2 + 10.3) |
//! | `get_land_cover_changes` | Compare per-location land-cover classes (10.4) |
//! | `get_tree_count_changes` | Compare per-location tree counts (10.5) |
//! | `get_change_magnitudes` | Per-location change magnitude and level (10.6) |
//! | `get_significant_change_areas` | Flag locations above the magnitude threshold (10.7) |
//! | `compare_sessions` | Run a comparison between two sessions' analysis results (vegetation coverage, land-cover, tree count) and produce a temporal result |
//! | `get_change_summary` | Return the latest computed temporal summary for a project, for Module 11 to consume |
//!
//! **Status**: 10.1 - 10.8 implemented. 10.1 - 10.3 read Module 1.4 images and
//! their pixels; 10.4 - 10.7 are per-location pure functions exposed as
//! commands, with the location maps produced by Module 3/4/5. `compare_sessions`
//! and `get_change_summary` remain `todo!()` stubs until those modules publish
//! their results.
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
pub mod map_controller;
mod modules;
mod plot;
mod plugin_manager;

use crate::map_controller::sp_measurement::spatial_rs::dummy::get_dummy_spatial_layers;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use plugin_manager::executor::ActiveJobTracker;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::sync::RwLock;

/// Shared application state injected into every Tauri command via `.manage()`.
#[derive(Clone)]
pub struct AppState {
    /// Active and recently completed plugin jobs (Role 1 — executor.rs).
    pub jobs: Arc<RwLock<ActiveJobTracker>>,
    /// SQLite connection shared across commands that need persistence
    /// (currently: Flight Session Management, Module 1.4).
    ///
    /// Persisted at `<app-data-dir>/aerial.db` (overridable via `AAP_DB_PATH`).
    /// Moved from the temporary working-directory placeholder (`./aerial.db`)
    /// to an absolute OS-standard path so library cleanup sweeps and data
    /// access remain stable regardless of the process launch directory.
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// App-owned root of imported images, decided once at startup.
    ///
    /// Imported files are **copied** below it, so a stored image survives the
    /// user moving or deleting the folder it came from. Override with the
    /// `AAP_LIBRARY_ROOT` environment variable; otherwise this is
    /// `<app-data-dir>/library`. See
    /// [`modules::module_01::import_io::session_images_dir`] for the layout.
    pub library_root: Arc<PathBuf>,
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

/// Makes `path` absolute without requiring it to exist yet.
fn absolute_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|err| format!("cannot determine the current working directory: {err}"))
    }
}

/// Where the app's SQLite database lives this start, as a decision the
/// caller still acts on (creating parents, copying a legacy file, opening).
#[derive(Debug)]
enum DbLocation {
    /// `AAP_DB_PATH` was set — an absolute path. It wins over everything.
    Override(PathBuf),
    /// `<app-data-dir>/aerial.db` exists: it stays the database. A legacy
    /// working-directory file (if any) is older state and is ignored.
    Resident(PathBuf),
    /// Fresh install target; no database file anywhere yet.
    Fresh(PathBuf),
    /// The target does not exist but the working directory still holds the
    /// pre-M1-placeholder `./aerial.db`: copy it (never move it) so the
    /// session history survives the migration to the app-data directory.
    Adopted {
        /// Where the database will live from now on.
        target: PathBuf,
        /// The legacy file the user data is copied from.
        legacy: PathBuf,
    },
}

/// Decides the database location, keeping the copy policy behind a pure fn.
///
/// Rules, in order: an explicit `AAP_DB_PATH` env override wins; then an
/// existing database at the app-data location wins; then a legacy
/// `./aerial.db` in the working directory is adopted by copying; otherwise
/// the app-data location starts fresh. On [`DbLocation::Adopted`] both paths
/// are returned so the caller can log and copy explicitly.
fn plan_db_location(
    cwd: &std::path::Path,
    app_data: &std::path::Path,
    env: Option<PathBuf>,
) -> DbLocation {
    if let Some(override_path) = env {
        return DbLocation::Override(override_path);
    }
    let target = app_data.join("aerial.db");
    let legacy = cwd.join("aerial.db");
    if target.is_file() {
        return DbLocation::Resident(target);
    }
    if legacy.is_file() {
        return DbLocation::Adopted { target, legacy };
    }
    DbLocation::Fresh(target)
}

/// Copies a legacy database from the working directory into the app-data
/// target, creating the target's parent directory if it does not exist yet.
///
/// Returns `true` if the copy succeeded, `false` otherwise.
fn adopt_legacy_db(target: &std::path::Path, legacy: &std::path::Path) -> bool {
    if let Some(parent) = target.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = std::fs::create_dir_all(parent) {
                eprintln!(
                    "warning: could not create the app-data directory for 'aerial.db': {err}"
                );
                return false;
            }
        }
    }
    // Copy, never move: if the copy fails mid-way, the user
    // must be able to retry by hand. Logs carry names only.
    match std::fs::copy(legacy, target) {
        Ok(_) => {
            eprintln!(
                "info: copied the legacy database 'aerial.db' found in the working directory into the app-data directory"
            );
            true
        }
        Err(err) => {
            eprintln!(
                "warning: could not copy the legacy database 'aerial.db'; starting with an empty database in the app-data directory: {err}"
            );
            false
        }
    }
}

/// One background library sweep: read the database, then clean the library.
///
/// Extracted from the startup closure so the e2e tests can exercise the exact
/// same path. `min_age` is [`modules::module_01::import_io::ORPHAN_MIN_AGE`]
/// in production and `Duration::ZERO` in tests; `0` in the count means the
/// sweep was skipped (no image rows) and nothing was trashed.
pub(crate) fn run_library_sweep(
    db: &std::sync::Mutex<rusqlite::Connection>,
    root: &std::path::Path,
    min_age: std::time::Duration,
) -> usize {
    use modules::module_01::{flight_session, import_io};
    let (referenced, known_scopes) = match db.lock() {
        Ok(conn) => {
            let paths = match flight_session::all_image_paths(&conn) {
                Ok(paths) => paths,
                Err(err) => {
                    eprintln!("warning: skipped the library sweep: {err}");
                    return 0;
                }
            };
            let scopes = match flight_session::known_session_scopes(&conn) {
                Ok(scopes) => scopes,
                Err(err) => {
                    eprintln!("warning: skipped the library sweep: {err}");
                    return 0;
                }
            };
            (paths, scopes)
        }
        Err(err) => {
            eprintln!("warning: skipped the library sweep: {err}");
            return 0;
        }
    };
    import_io::cleanup_orphaned_copies_older_than(&referenced, &known_scopes, root, min_age)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Resolved here, not in the command, so every import in this
            // process writes below the same root. There is no silent cwd
            // fallback: an import that later cannot be found again is worse
            // than a startup error naming the real problem.
            let app_data = app
                .path()
                .app_data_dir()
                .map_err(|err| format!("cannot resolve the app-data directory: {err}"))?;
            // Absolute, never `aerial.db` in whatever directory the process
            // was started from. A relative database plus a sweep that deletes
            // unreferenced files is how the library gets wiped.
            let cwd = std::env::current_dir()
                .map_err(|err| format!("cannot determine the current working directory: {err}"))?;
            let db_path = match plan_db_location(
                &cwd,
                &app_data,
                std::env::var_os("AAP_DB_PATH").map(PathBuf::from),
            ) {
                DbLocation::Override(path) => path,
                DbLocation::Resident(path) => path,
                DbLocation::Fresh(path) => path,
                DbLocation::Adopted { target, legacy } => {
                    adopt_legacy_db(&target, &legacy);
                    target
                }
            };
            if let Some(parent) = db_path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)
                        .map_err(|err| format!("cannot create '{}': {err}", parent.display()))?;
                }
            }
            let db_conn = rusqlite::Connection::open(&db_path)
                .map_err(|err| format!("failed to open '{}': {err}", db_path.display()))?;
            // A schema that cannot be initialised must not stop the desktop
            // app from opening: Modules 2-11 do not need it.
            if let Err(err) = modules::module_01::init_schema(&db_conn) {
                eprintln!("warning: could not initialize the session/image schema: {err}");
            }
            let library_root = match std::env::var_os("AAP_LIBRARY_ROOT") {
                Some(value) => absolute_path(PathBuf::from(value))?,
                None => app_data.join("library"),
            };
            let db = Arc::new(Mutex::new(db_conn));
            let library_root = Arc::new(library_root);
            let sweep_db = Arc::clone(&db);
            let sweep_root = Arc::clone(&library_root);
            app.manage(AppState {
                jobs: Arc::new(RwLock::new(ActiveJobTracker::new())),
                db,
                library_root,
            });
            // Off the startup path, and the database lock is dropped before
            // the walk: a large library must not delay launch or stall
            // session commands. The sweep only touches the app's own
            // `projects/.../images` layout for sessions that exist in SQLite,
            // moves orphans to `.trash/` instead of deleting them, and skips
            // itself when the database has no image rows.
            std::thread::spawn(move || {
                let trashed = run_library_sweep(
                    &sweep_db,
                    &sweep_root,
                    modules::module_01::import_io::ORPHAN_MIN_AGE,
                );
                if trashed > 0 {
                    eprintln!("info: moved {trashed} orphaned image copies to .trash");
                }
            });
            Ok(())
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
            commands::session::get_images_by_session,
            commands::session::get_image_metadata,
            commands::session::assign_image_to_session,
            commands::session::recalculate_session_date_range,
            commands::session::delete_session,
            commands::session::delete_image,
            commands::session::move_image_to_session,
            commands::session::copy_image_to_session,
            // Image import & preview
            commands::import::import_images,
            commands::import::get_analysis_input,
            commands::image_preview::preview_image,
            commands::image_preview::thumbnail_image,
            // Project management
            commands::project::create_project,
            commands::project::get_project,
            commands::project::list_projects,
            commands::project::update_project,
            commands::project::delete_project,
            commands::project::count_images_by_project,
            // Temporal change analysis (Module 10)
            modules::module_10::commands::get_observation_periods,
            modules::module_10::commands::get_vegetation_change,
            modules::module_10::commands::get_land_cover_changes,
            modules::module_10::commands::get_tree_count_changes,
            modules::module_10::commands::get_change_magnitudes,
            modules::module_10::commands::get_significant_change_areas,
            modules::module_10::commands::compare_sessions,
            modules::module_10::commands::get_change_summary,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{adopt_legacy_db, plan_db_location, DbLocation};

    /// The team decision review called for a test-backed copy policy: an
    /// existing `./aerial.db` must survive the move to the app-data
    /// directory, and the app-data file must never be clobbered.
    #[test]
    fn legacy_db_is_copied_when_target_missing() {
        let root = tempfile::tempdir().expect("root");
        let cwd = root.path().join("cwd");
        let app_data = root.path().join("app-data");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&app_data).expect("app data");
        std::fs::write(cwd.join("aerial.db"), b"history").expect("legacy db");

        match plan_db_location(&cwd, &app_data, None) {
            DbLocation::Adopted { target, legacy } => {
                assert_eq!(target, app_data.join("aerial.db"));
                assert_eq!(legacy, cwd.join("aerial.db"));
                assert!(adopt_legacy_db(&target, &legacy));
                assert_eq!(std::fs::read(&target).expect("read target"), b"history");
            }
            other => panic!("expected an adopted legacy database, got {other:?}"),
        }
    }

    /// On a first launch, the app-data directory does not exist on disk yet.
    /// `adopt_legacy_db` must ensure the parent directory is created before
    /// copying so the legacy history is not lost.
    #[test]
    fn legacy_db_is_adopted_when_app_data_does_not_exist_beforehand() {
        let root = tempfile::tempdir().expect("root");
        let cwd = root.path().join("cwd");
        let app_data = root.path().join("app-data");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::write(cwd.join("aerial.db"), b"history").expect("legacy db");

        assert!(!app_data.exists(), "app_data dir must not exist beforehand");

        match plan_db_location(&cwd, &app_data, None) {
            DbLocation::Adopted { target, legacy } => {
                assert_eq!(target, app_data.join("aerial.db"));
                assert_eq!(legacy, cwd.join("aerial.db"));
                assert!(adopt_legacy_db(&target, &legacy));
                assert!(target.is_file(), "target database was copied successfully");
                assert_eq!(std::fs::read(&target).expect("read target"), b"history");
                assert_eq!(
                    std::fs::read(&legacy).expect("legacy file preserved"),
                    b"history"
                );
            }
            other => panic!("expected an adopted legacy database, got {other:?}"),
        }
    }

    #[test]
    fn existing_target_is_never_overwritten() {
        let root = tempfile::tempdir().expect("root");
        let cwd = root.path().join("cwd");
        let app_data = root.path().join("app-data");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&app_data).expect("app data");
        std::fs::write(cwd.join("aerial.db"), b"legacy").expect("legacy db");
        std::fs::write(app_data.join("aerial.db"), b"current").expect("resident db");

        match plan_db_location(&cwd, &app_data, None) {
            DbLocation::Resident(path) => assert_eq!(path, app_data.join("aerial.db")),
            other => panic!("expected the resident database to win, got {other:?}"),
        }
        assert_eq!(
            std::fs::read(cwd.join("aerial.db")).expect("legacy untouched"),
            b"legacy"
        );
        assert_eq!(
            std::fs::read(app_data.join("aerial.db")).expect("resident untouched"),
            b"current"
        );
    }

    #[test]
    fn env_override_wins() {
        let root = tempfile::tempdir().expect("root");
        let cwd = root.path().join("cwd");
        let app_data = root.path().join("app-data");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&app_data).expect("app data");
        let custom = root.path().join("custom").join("main.db");
        std::fs::write(cwd.join("aerial.db"), b"legacy").expect("legacy db");
        std::fs::write(app_data.join("aerial.db"), b"resident").expect("resident db");

        match plan_db_location(&cwd, &app_data, Some(custom.clone())) {
            DbLocation::Override(path) => assert_eq!(path, custom),
            other => panic!("expected the env override to win, got {other:?}"),
        }
    }

    #[test]
    fn fresh_install_is_the_app_data_location() {
        let root = tempfile::tempdir().expect("root");
        let cwd = root.path().join("cwd");
        let app_data = root.path().join("app-data");
        std::fs::create_dir_all(&cwd).expect("cwd");
        std::fs::create_dir_all(&app_data).expect("app data");

        match plan_db_location(&cwd, &app_data, None) {
            DbLocation::Fresh(path) => assert_eq!(path, app_data.join("aerial.db")),
            other => panic!("expected a fresh install, got {other:?}"),
        }
    }
}
