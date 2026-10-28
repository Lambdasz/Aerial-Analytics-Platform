# Project Structure: Module 03 (Aerial Image Map Explorer)

This document provides a comprehensive structural inventory of all files, folders, and storage paths associated with **Module 03 (Aerial Image Map Explorer)**.

---

## 1. Backend Rust Source Tree (`src-tauri/src/modules/module_03/`)

The backend source code for Module 03 is organized modularly under `src-tauri/src/modules/module_03/`. Business logic is partitioned into dedicated domain submodules:

```text
src-tauri/src/modules/module_03/
├── mod.rs                        # Root facade module, submodules declaration, and pub use re-exports
├── storage.rs                    # Direct filesystem storage primitives and folder initialization
├── storage_commands.rs           # Re-exported Tauri IPC command facade for storage operations
├── storage_utils.rs              # Path resolution utilities (get_storage_root, sanitize_filename)
├── error.rs                      # MapControllerError and OrthoImportError enum definitions
├── state.rs                      # Module03State in-memory write-through cache and synchronizer
├── types.rs                      # Common shared Rust structs across Module 03 domains
├── aoi/                          # Area of Interest (AOI) domain
│   ├── mod.rs                    # Submodule definition and public exports
│   ├── commands.rs               # Tauri IPC command handlers for AOI operations
│   ├── store.rs                  # Filesystem persistence, CRUD, and metric recalculations
│   ├── types.rs                  # AOI data structs (AoiPolygon, AoiFeature, AoiMetrics)
│   └── builder.rs                # Pure geometric polygon construction and metric math
├── annotations/                  # Spatial Annotations domain
│   ├── mod.rs                    # Submodule definition and public exports
│   ├── commands.rs               # Tauri IPC command handlers for annotations
│   ├── store.rs                  # Filesystem persistence for annotations in map_storage/annotations/
│   └── types.rs                  # AnnotationFeature, AnnotationProperties, and GeoJSON shapes
├── geometry/                     # Geospatial Geometry calculations & validation
│   ├── mod.rs                    # Submodule definition, exports, and unit tests
│   ├── calculations.rs           # Shoelace area, geodesic perimeter, and centroid math
│   └── validation.rs             # WGS84 coordinate range and polygon topology validators
├── layers/                       # Map Layers metadata and rendering state
│   ├── mod.rs                    # Submodule definition and public exports
│   ├── commands.rs               # Tauri IPC command handlers for layer state
│   ├── store.rs                  # Filesystem persistence for layers in map_storage/layers/
│   ├── types.rs                  # LayerState, LayerType, and display properties
│   └── pure.rs                   # Pure transformation functions on layer collections
├── ortho_source/                 # Orthomosaic GeoTIFF & raster tiling engine
│   ├── mod.rs                    # Submodule definition and public exports
│   ├── commands.rs               # Tauri IPC commands for ortho import, listing, and inspection
│   ├── store.rs                  # Image directory management, meta.json, and cascade delete
│   ├── types.rs                  # StoredImageMetadata, ImportDualModePayload, JobRecord
│   ├── instance_lock.rs          # Process concurrency lock (.app.lock) and single-instance guard
│   ├── job_manager.rs            # Active background job tracking and CancellationToken registry
│   ├── geotiff_reader.rs         # Pure Rust GeoTIFF header parsing, tags, and CRS detection
│   ├── projection.rs             # UTM to WGS84 coordinate conversion math
│   ├── tiler.rs                  # Deep pyramid tile generator (zoom levels 10-18)
│   └── image_pipeline.rs         # Async background import supervisor for dual-mode ingestion
├── photos/                       # Drone photo capture markers domain
│   ├── mod.rs                    # Submodule definition and public exports
│   ├── commands.rs               # Tauri IPC commands for photo marker listings
│   ├── store.rs                  # Filesystem reader for drone photos
│   └── types.rs                  # PhotoMetadata, DroneImageMetadata, GPS coordinates
└── sp_result/                    # Spatial Results inspection domain
    ├── mod.rs                    # Submodule definition and public exports
    ├── commands.rs               # Tauri IPC commands for spatial analysis result CRUD
    ├── types.rs                  # SpatialResultItem, FeatureEditPayload, DisplayPreference
    ├── reader.rs                 # Disk scanning, JSON parsing, and directory tree ingestion
    ├── writer.rs                 # Atomic serialization and disk writing for results
    ├── patch.rs                  # In-place property patching, display preference, and photo sync
    └── cleanup.rs                # Quarantine management (.trash_*) and cascading removal
```

### Purpose of Backend Files

| Directory / File      | Domain / Scope    | Responsibilities & Architectural Role                                    |
| :-------------------- | :---------------- | :----------------------------------------------------------------------- |
| `mod.rs`              | Root Facade       | Declares all 10 submodules, applies crate lints, and exposes public APIs |
| `storage.rs`          | Storage Primitive | Ensures root directory creation (`aois/`, `images/`, etc.)               |
| `storage_commands.rs` | IPC Facade        | Exposes `get_storage_root_path` and re-exports domain storage commands   |
| `storage_utils.rs`    | Path Resolution   | Computes canonical storage roots and sanitizes unsafe paths              |
| `error.rs`            | Error Modeling    | Maps domain errors to `CommandError` across Tauri IPC                    |
| `state.rs`            | Memory Cache      | Thread-safe `Arc<RwLock<HashMap>>` cache with write-through disk sync    |
| `types.rs`            | Common Models     | Shared basic structs (e.g. `ImportBoundsPayload`)                        |
| `aoi/*`               | AOI Submodule     | Creates, recalculates metrics, validates, and persists user AOI polygons |
| `annotations/*`       | Annotations       | Manages vector annotations (point, linestring, polygon) with 64KB limits |
| `geometry/*`          | Geometry Math     | Pure calculations for geodesic area, perimeter, and bounding boxes       |
| `layers/*`            | Layer State       | Controls layer visibility, opacity, and stack ordering persistence       |
| `ortho_source/*`      | Orthomosaics      | GeoTIFF validation, dual-mode tiling, job tracking, and process lock     |
| `photos/*`            | Drone Photos      | Inspects and lists geotagged drone capture markers                       |
| `sp_result/*`         | Spatial Results   | Loads detection/classification results, patches properties, syncs images |

---

## 2. Frontend React Source Tree (`src/modules/module-03/`)

The frontend application for Module 03 resides in `src/modules/module-03/`, structured strictly according to **Atomic Design** principles:

```text
src/modules/module-03/
├── index.ts                      # Module entrypoint re-exporting primary components and hooks
├── REQUIREMENTS.md               # Feature requirements specification
├── RULES_MAP_FR.md               # Frontend coding guidelines (Leaflet, state, atomic design)
├── atoms/                        # Smallest atomic UI primitives
│   └── StatusBadge.tsx           # Status badge displaying online, active, or sync state
├── molecules/                    # Simple UI component compositions
│   ├── AoiCardItem.tsx           # Individual AOI card in sidebar with zoom/delete actions
│   ├── AoiCreateForm.tsx         # Inline form for naming and styling draft AOI polygon
│   ├── AoiDetailModal.tsx        # Modal presenting detailed metrics (area, perimeter, coordinates)
│   ├── DeleteAoiDialog.tsx       # Confirmation dialog for AOI deletion
│   ├── DeleteConfirmDialog.tsx   # Generic confirmation dialog for destructive actions
│   ├── EditAoiModal.tsx          # Modal for editing existing AOI name and properties
│   ├── EditSpResultModal.tsx     # Comprehensive editor for spatial results and photo synchronization
│   ├── FeatureDetailCard.tsx     # Feature property inspector card in sidebar
│   ├── LayerCardItem.tsx         # Layer item in sidebar with visibility/opacity toggles
│   ├── MapSearchBar.tsx          # Search bar input for locating coordinates or features
│   ├── MeasureResultCard.tsx     # Floating card displaying active distance/area measurements
│   ├── OrthoCardItem.tsx         # Orthomosaic card showing thumbnail, dimensions, and actions
│   ├── OrthoDeleteModal.tsx      # Cascade deletion summary and confirmation modal
│   ├── OrthoDetailModal.tsx      # Technical metadata viewer for orthomosaic files
│   ├── OrthoEditModal.tsx        # Modal for adjusting orthomosaic name and bounds
│   ├── OrthoUploadModal.tsx      # Dual-mode modal for uploading and tiling orthomosaics
│   ├── PhotoCardItem.tsx         # Thumbnail card for individual geotagged drone photos
│   ├── ResultPhoto.tsx           # Standardized image component with fallback handling
│   └── SpatialResultItem.tsx     # Tree node item representing spatial analysis layers
├── organisms/                    # Complex UI sections composed of molecules and atoms
│   ├── AoiToolbar.tsx            # Left toolbar managing AOI creation, list, and tools
│   ├── DrawingControlBar.tsx     # Geoman drawing controls overlay for map canvas
│   ├── ImageGallerySidebar.tsx   # Sidebar panel browsing geotagged capture photos
│   ├── LayerManager.tsx          # Layer hierarchy, opacity sliders, and annotation list
│   ├── ManagedMapLayers.tsx      # Declarative Leaflet layer renderer for vectors and rasters
│   ├── MapCanvas.tsx             # Interactive Leaflet map container and tile setup
│   ├── MapLeftPanel.tsx          # Tabbed left sidebar container (AOI, Layers, Results, Ortho)
│   ├── MapViewControlBar.tsx     # Floating map toolbar (zoom, center, measure, basemap)
│   ├── OrthoImportPanel.tsx      # In-panel orthomosaic upload and job progress monitor
│   ├── SpatialResultDetail.tsx   # Detailed inspection view for a selected spatial result
│   └── SpatialResultTree.tsx     # Hierarchical tree view of spatial classification results
├── pages/                        # Page-level compositions
│   └── MapPage.tsx               # Main page hosting map canvas, controls, and left drawer
├── hooks/                        # Custom React hooks encapsulating state and side-effects
│   ├── useAOI.ts                 # Full lifecycle state hook for Area of Interest polygons
│   ├── useAnnotations.ts         # Hook managing user vector annotations
│   ├── useImageMarkers.ts        # Hook loading and caching drone photo GPS points
│   ├── useLeafletGeoman.ts       # Integration hook connecting Leaflet-Geoman toolbar events
│   └── useSpatialResultLayers.ts # Hook fetching, formatting, and toggling spatial results
├── services/                     # IPC and network abstraction layer
│   ├── api.ts                    # 38 typed functions wrapping Tauri invokeMap calls
│   └── error.ts                  # Standardized invokeMap wrapper and error normalization
├── types/                        # TypeScript domain interfaces and type definitions
│   ├── aoi.ts                    # AOI entity and geometry definitions
│   ├── index.ts                  # Central re-exports of all types
│   ├── layers.ts                 # LayerState, LayerType, and visibility interfaces
│   ├── map.ts                    # Leaflet bounds, coordinates, and view configurations
│   ├── ortho.ts                  # Orthomosaic metadata, upload payloads, and job records
│   └── spatialResult.ts          # Spatial result, feature properties, and display styles
├── lib/                          # Pure algorithmic helpers and utility functions
│   ├── aoiCalculations.ts        # Client-side area, perimeter, and bounding box math
│   ├── fieldResolution.ts        # Attribute resolution, display naming, and image URL resolution
│   ├── geoJson.ts                # GeoJSON normalization and coordinate conversions
│   ├── measurement.ts            # Dynamic measurement calculation during drawing
│   ├── popupBuilder.ts           # HTML popup builders for map markers and polygons
│   └── leaflet/                  # Leaflet-specific geometry builders
│       ├── aoiLayerBuilder.ts    # Renders interactive Leaflet polygon overlays for AOI
│       └── spatialLayerBuilder.ts # Converts GeoJSON features into styled Leaflet vector layers
├── config/                       # Static configurations
│   └── mapConfig.ts              # Basemap tile providers (OSM, Satellite) and default zoom
├── store/                        # Cross-component state stores
│   └── ImageSyncContext.tsx      # Context provider synchronizing active image highlights
└── docs/                         # Internal specifications and reference documents
    ├── Proyek.md                 # PBL 2026 project specification copy
    ├── README_UMUM.md            # General overview of aerial visualization
    └── README_modul3.md          # Technical notes on Module 3 requirements
```

### Purpose of Frontend Categories

| Folder / Layer | Responsibility         | Key Rules & Constraints                                     |
| :------------- | :--------------------- | :---------------------------------------------------------- |
| `atoms/`       | Elementary UI elements | Completely stateless; receive props and render UI           |
| `molecules/`   | Combinations of atoms  | Focused on a single task (e.g. card item, modal form)       |
| `organisms/`   | Composite components   | Coordinate state hooks with child molecules; emit actions   |
| `pages/`       | Page layout            | Arranges organisms into full-screen responsive workflows    |
| `hooks/`       | Business & Map State   | Encapsulates React state, memoized callbacks, and IPC calls |
| `services/`    | Tauri IPC Transport    | All communication routes through `invokeMap` in `error.ts`  |
| `lib/`         | Pure Algorithms        | Deterministic utility functions without UI or side effects  |

---

## 3. Storage Filesystem Tree (`map_storage/`)

Module 03 relies on a dedicated local storage directory named `map_storage/` at the workspace root. All write operations enforce single-instance locking and atomic write-replace semantics:

```text
map_storage/
├── .app.lock                     # Concurrency control lock file containing instance PID
├── .jobs/                        # Background import job status records
│   └── {job_id}.json             # Job progress, status, timestamps, and error logs
├── aois/                         # Saved Area of Interest definitions
│   └── {aoi_id}.json             # GeoJSON polygon geometry, properties, and area metrics
├── annotations/                  # User drawn vector annotations
│   └── {annotation_id}.json      # GeoJSON feature with notes, tags, and category
├── images/                       # Orthomosaic raster storage directory
│   └── {image_id}/               # Directory dedicated to a specific orthomosaic
│       ├── meta.json             # Image metadata (bounds, resolution, file size, status)
│       ├── raw.tif               # Original GeoTIFF raster file
│       └── tiles/                # Deep zoom pyramid tile structure
│           └── {z}/{x}/{y}.png   # Slippy map raster tiles generated for Web Mercator
├── layers/                       # Map layer configurations
│   └── {layer_id}.json           # Layer visibility, opacity, type, and source bindings
├── photos/                       # Geotagged drone capture photos
│   └── {photo_id}.json           # Photo metadata, altitude, yaw, pitch, and GPS coords
├── spatial_results/              # Spatial detection & classification results
│   └── {result_id}.json          # Single-file or manifest result definition
│   └── {result_id}/              # Multi-feature spatial result directory
│       ├── manifest.json         # Result metadata, schema version, and display preference
│       ├── result_image/         # Extracted or associated result images
│       │   └── {image_name}.png  # Raster image or overlay thumbnail
│       └── features/             # Individual detected features
│           └── {feature_id}.json # GeoJSON feature with bounding boxes, classes, and confidence
├── .trash_{timestamp}_{id}/      # Quarantined directories awaiting permanent deletion
└── temp_{uuid}.tmp               # Atomic write buffer files before rename
```

### Storage Paths Lifecycle & Ownership Matrix

| Path Pattern                         | Format     | Producer (Writer)       | Consumer (Reader)       | Retention & Lifecycle Rules                                         |
| :----------------------------------- | :--------- | :---------------------- | :---------------------- | :------------------------------------------------------------------ |
| `map_storage/.app.lock`              | Text (PID) | `instance_lock.rs`      | All instances           | Created on launch; deleted on clean exit; checked via PID heartbeat |
| `map_storage/.jobs/{job_id}.json`    | JSON       | `image_pipeline.rs`     | `commands.rs`, UI       | Created at start of import; updated on progress; kept for history   |
| `map_storage/aois/{id}.json`         | JSON       | `aoi/store.rs`          | `aoi/store.rs`, UI      | Persisted on save; updated on edit; removed on user delete          |
| `map_storage/annotations/{id}.json`  | JSON       | `annotations/store.rs`  | `annotations/store.rs`  | Persisted on save; size limited to 64 KB; deleted on user action    |
| `map_storage/images/{id}/meta.json`  | JSON       | `ortho_source/store.rs` | `ortho_source/store.rs` | Created during import; purged on cascade image deletion             |
| `map_storage/images/{id}/raw.tif`    | GeoTIFF    | `image_pipeline.rs`     | Tile generator          | Ingested on import; retained as original raster source              |
| `map_storage/images/{id}/tiles/*`    | PNG        | `tiler.rs`              | Leaflet TileLayer       | Generated during deep tiling; cached for offline map viewing        |
| `map_storage/layers/{id}.json`       | JSON       | `layers/store.rs`       | `layers/store.rs`       | Persisted on layer creation; auto-healed or pruned on startup       |
| `map_storage/photos/{id}.json`       | JSON       | External / Plugins      | `photos/store.rs`       | Read by photo gallery; read-only during standard exploration        |
| `map_storage/spatial_results/*.json` | JSON       | Plugins / `writer.rs`   | `reader.rs`, UI         | Ingested from plugins; properties editable via UI patch             |
| `map_storage/.trash_*`               | Directory  | `ortho_source/store.rs` | `cleanup.rs`            | Quarantine folder; permanently removed after layer sync completes   |
| `map_storage/temp_*`                 | Temp File  | `storage.rs`            | Target file             | Ephemeral atomic buffer; immediately renamed to final destination   |
