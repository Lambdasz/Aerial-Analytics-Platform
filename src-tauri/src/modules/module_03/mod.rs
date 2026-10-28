//! # Module 03: Aerial Image Map Explorer
//!
//! Module 03 provides the interactive geospatial exploration engine for the Aerial Analytics Platform.
//! It is responsible for rendering high-resolution drone orthomosaics, managing Area of Interest (AOI)
//! vector boundaries, inspecting spatial AI detection and classification results, managing user annotations,
//! and supervising disk-backed single-instance persistence under `map_storage/`.
//!
//! ## 1. Source Code Trees
//!
//! ### Backend Source Tree (`src-tauri/src/modules/module_03/`)
//!
//! ```text
//! src-tauri/src/modules/module_03/
//! ├── mod.rs                        # Root facade, submodule declarations, and architectural documentation
//! ├── storage.rs                    # Filesystem storage primitives and folder initialization
//! ├── storage_commands.rs           # Tauri IPC command facade re-exporting storage operations
//! ├── storage_utils.rs              # Path resolution utilities and filename sanitization
//! ├── error.rs                      # MapControllerError and OrthoImportError domain errors
//! ├── state.rs                      # Module03State in-memory write-through cache
//! ├── types.rs                      # Common shared Rust structs across Module 03
//! ├── aoi/                          # Area of Interest (AOI) domain (commands, store, builder, types)
//! ├── annotations/                  # User vector annotations domain (commands, store, types)
//! ├── geometry/                     # Geospatial math (calculations, validation, projection)
//! ├── layers/                       # Map layers metadata and rendering state (commands, store, types)
//! ├── ortho_source/                 # Orthomosaic GeoTIFF & raster tiling engine
//! ├── photos/                       # Geotagged drone photo capture markers (commands, store, types)
//! └── sp_result/                    # Spatial AI analysis results domain (reader, writer, patch, cleanup)
//! ```
//!
//! ### Frontend Source Tree (`src/modules/module-03/`)
//!
//! ```text
//! src/modules/module-03/
//! ├── index.ts                      # Module entrypoint re-exporting primary components and hooks
//! ├── atoms/                        # Stateless UI primitives (StatusBadge)
//! ├── molecules/                    # Simple UI compositions (AoiCardItem, OrthoCardItem, ResultPhoto)
//! ├── organisms/                    # Complex units (AoiToolbar, LayerManager, MapCanvas, MapLeftPanel)
//! ├── pages/                        # Full layout container (MapPage)
//! ├── hooks/                        # Custom React hooks (useAOI, useAnnotations, useSpatialResultLayers)
//! ├── services/                     # IPC transport layer (api.ts wrapping invokeMap in error.ts)
//! ├── types/                        # TypeScript domain interfaces
//! ├── lib/                          # Pure algorithms (aoiCalculations, fieldResolution, geoJson)
//! ├── config/                       # Basemap tile provider configurations
//! └── store/                        # Cross-component state stores (ImageSyncContext)
//! ```
//!
//! ## 2. Architecture & Layered Model
//!
//! Module 03 enforces a strict unidirectional layered architecture:
//!
//! ```text
//! +---------------------------------------------------------------+
//! |              Tauri IPC Boundary (map_controller)              |
//! |  Thin handlers: validate inputs, enforce instance lock, map   |
//! +---------------------------------------------------------------+
//!                                |
//!                                v
//! +---------------------------------------------------------------+
//! |               Domain Service & Pipeline Layer                 |
//! |  geotiff_reader.rs, image_pipeline.rs, aoi/builder.rs        |
//! +---------------------------------------------------------------+
//!                                |
//!                                v
//! +---------------------------------------------------------------+
//! |               Storage & Memory Cache Layer                    |
//! |  store.rs, reader.rs, writer.rs, state.rs (write-through)     |
//! +---------------------------------------------------------------+
//!                                |
//!                                v
//! +---------------------------------------------------------------+
//! |                 Disk Storage (map_storage/)                   |
//! |  Atomic persistence via write_atomic (temp_* -> rename)       |
//! +---------------------------------------------------------------+
//! ```
//!
//! ## 3. Storage Hierarchy (`map_storage/`)
//!
//! ```text
//! map_storage/
//! ├── .app.lock                     # Concurrency lock file storing primary PID
//! ├── .jobs/                        # Background import job records ({job_id}.json)
//! ├── aois/                         # Saved Area of Interest polygons ({id}.json)
//! ├── annotations/                  # User vector annotations ({id}.json, max 64 KB)
//! ├── images/{id}/                  # Orthomosaics (meta.json, raw.tif, tiles/{z}/{x}/{y}.png)
//! ├── layers/                       # Map layer configurations ({id}.json)
//! ├── photos/                       # Geotagged drone capture photos ({id}.json)
//! ├── spatial_results/              # AI detection/classification results (manifest.json, features/)
//! ├── .trash_*                      # Quarantined directories awaiting permanent deletion
//! └── temp_*                        # Ephemeral atomic write buffer files before rename
//! ```
//!
//! ## 4. Tauri IPC Command Index
//!
//! The following 39 Tauri IPC commands are registered under `map_controller`:
//!
//! - **Geometry & Measurement**:
//!   - [`measure_distance`](geometry::commands::measure_distance) — Computes distance across coordinate points.
//!   - [`measure_area`](geometry::commands::measure_area) — Computes Shoelace area for polygon coordinates.
//!   - [`measure_perimeter`](geometry::commands::measure_perimeter) — Computes geodesic perimeter for polygon.
//! - **Area of Interest (AOI)**:
//!   - [`create_aoi_cmd`](aoi::commands::create_aoi_cmd) — Validates polygon and builds draft AOI feature.
//!   - [`save_new_aoi_cmd`](aoi::commands::save_new_aoi_cmd) — Constructs, validates, and persists new AOI.
//!   - [`save_aoi`](aoi::commands::save_aoi) — Persists pre-constructed AOI feature to disk.
//!   - [`get_saved_aois`](aoi::commands::get_saved_aois) — Lists all stored AOI polygons.
//!   - [`delete_saved_aoi`](aoi::commands::delete_saved_aoi) — Deletes an AOI polygon from disk.
//! - **Map Layers**:
//!   - [`get_saved_layers`](layers::commands::get_saved_layers) — Lists all registered map layer records.
//! - **Vector Annotations**:
//!   - [`create_annotation_cmd`](annotations::commands::create_annotation_cmd) — Creates and persists user annotation.
//!   - [`get_saved_annotations`](annotations::commands::get_saved_annotations) — Lists all stored vector annotations.
//!   - [`get_annotation_by_id`](annotations::commands::get_annotation_by_id) — Fetches single annotation by ID.
//!   - [`update_annotation_cmd`](annotations::commands::update_annotation_cmd) — Updates an existing vector annotation.
//!   - [`delete_annotation_cmd`](annotations::commands::delete_annotation_cmd) — Removes vector annotation from storage.
//! - **Spatial AI Results**:
//!   - [`save_spatial_result`](sp_result::commands::save_spatial_result) — Persists AI classification or detection result.
//!   - [`get_saved_spatial_results`](sp_result::commands::get_saved_spatial_results) — Loads full spatial result trees.
//!   - [`get_saved_spatial_results_summary`](sp_result::commands::get_saved_spatial_results_summary) — Loads lightweight result summaries.
//!   - [`get_saved_spatial_result`](sp_result::commands::get_saved_spatial_result) — Loads single spatial result by ID.
//!   - [`update_saved_spatial_result`](sp_result::commands::update_saved_spatial_result) — Updates result metadata and syncs photos.
//!   - [`count_spatial_result_dependents`](sp_result::commands::count_spatial_result_dependents) — Counts dependent layers.
//!   - [`delete_saved_spatial_result`](sp_result::commands::delete_saved_spatial_result) — Deletes spatial result from storage.
//!   - [`check_spatial_result_manual_edits`](sp_result::commands::check_spatial_result_manual_edits) — Checks for manual edits.
//! - **Orthomosaic Rasters**:
//!   - [`list_ortho_images`](ortho_source::commands::list_ortho_images) — Lists all registered orthomosaics.
//!   - [`get_ortho_image`](ortho_source::commands::get_ortho_image) — Fetches single orthomosaic metadata.
//!   - [`update_ortho_image`](ortho_source::commands::update_ortho_image) — Updates name or bounds of orthomosaic.
//!   - [`get_ortho_cascade_summary`](ortho_source::commands::get_ortho_cascade_summary) — Summarizes dependent layers.
//!   - [`delete_ortho_image`](ortho_source::commands::delete_ortho_image) — Performs cascade deletion of orthomosaic.
//!   - [`save_image_from_path`](ortho_source::commands::save_image_from_path) — Deprecated stub directing to async job.
//!   - [`get_saved_images`](ortho_source::commands::get_saved_images) — Legacy listing of stored images.
//!   - [`delete_saved_image`](ortho_source::commands::delete_saved_image) — Legacy image deletion command.
//!   - [`import_ortho_image`](ortho_source::commands::import_ortho_image) — Legacy sync import command.
//!   - [`start_import_ortho_job`](ortho_source::commands::start_import_ortho_job) — Starts async dual-mode orthomosaic import.
//!   - [`cancel_import_job`](ortho_source::commands::cancel_import_job) — Cancels running import job.
//!   - [`get_import_job_status`](ortho_source::commands::get_import_job_status) — Polls job status from disk.
//!   - [`get_instance_status`](ortho_source::commands::get_instance_status) — Reports whether process is primary instance.
//!   - [`inspect_geotiff_header`](ortho_source::commands::inspect_geotiff_header) — Fast pre-validation of GeoTIFF header.
//! - **Drone Photos**:
//!   - [`get_photos`](photos::commands::get_photos) — Retrieves drone photo capture markers.
//!   - [`import_photo`](photos::commands::import_photo) — Ingests single drone photo into storage.
//! - **Storage Root**:
//!   - [`get_storage_root_path`](storage_commands::get_storage_root_path) — Canonical path of `map_storage`.
//!
//! ## 5. Data Flow Pipelines
//!
//! ### Storage to Frontend (Read Direction)
//!
//! ```text
//! [ Disk: map_storage/ ]
//!            |
//!            | fs::read_to_string()
//!            v
//! [ Domain Reader (reader.rs / store.rs) ]
//!   - Validates JSON format
//!   - Isolates corruption (is_corrupted = true)
//!            |
//!            v
//! [ Module03State Cache (state.rs) ]
//!   - Updates Arc<RwLock<HashMap<...>>>
//!            |
//!            v
//! [ Tauri IPC Command Handler ] -> [ Frontend invokeMap ]
//! ```
//!
//! ### Frontend to Storage (Write Direction)
//!
//! ```text
//! [ Frontend IPC Mutation Request ]
//!            |
//!            v
//! [ require_primary_instance Guard ]
//!   - Rejects secondary read-only windows
//!            |
//!            v
//! [ Parameter & Geometry Validation ]
//!   - Enforces WGS84 range & 64 KB JSON ceiling
//!            |
//!            v
//! [ write_atomic (storage.rs) ]
//!   - Writes temp_{uuid}.tmp -> flush -> atomic rename
//!            |
//!            v
//! [ Module03State Cache Update ] -> Return Success
//! ```
//!
//! ## 6. Error Models
//!
//! Domain errors are modeled cleanly and mapped to `CommandError` across IPC boundaries:
//! - [`error::MapControllerError`]: Core storage, geometry, and validation errors.
//! - [`error::OrthoImportError`]: Specialized errors during GeoTIFF reading, projection, or tiling.
//!
//! ## 7. Concurrency & Lifecycle
//!
//! - **Single-Instance Lock**: Managed by [`ortho_source::instance_lock`]. Primary process owns `map_storage/.app.lock`.
//! - **Job Registry**: Background tasks are coordinated via [`ortho_source::job_manager`] using `CancellationToken`.
//! - **Startup Reconcile**: Handled by [`ortho_source::store::reconcile_storage_layers`], resolving `.trash_*` folders and auto-healing layers.
//!
//! ## 8. Engineering Conventions
//!
//! - **Atomic Writes**: Every write MUST use `write_atomic` to prevent half-written files.
//! - **Special Folders**: Dot-prefixed folders (`.jobs/`, `.trash_*`) indicate internal runtime metadata.
//! - **Size Limits**: Vector JSON payloads are capped at 64 KB; rasters larger than 4,096px trigger Mode B tiling.

pub mod annotations;
pub mod aoi;
pub mod commands;
pub mod error;
pub mod geometry;
pub mod layer;
pub mod layer_commands;
pub mod layers;
pub mod ortho_source;
pub mod photos;
pub mod sp_measurement;
pub mod sp_result;
pub mod state;
pub mod storage;
pub mod storage_commands;
pub mod storage_utils;
pub mod types;
