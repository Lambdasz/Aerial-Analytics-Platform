# Spatial Results Specification & Storage Architecture (Module 03)

This document provides a comprehensive technical guide to the `Spatial Result` data contract, JSON authoring rules, and the storage, mutation, and lifecycle mechanisms within **Module 03 (Aerial Image Map Explorer)** of the Aerial Analytics Platform.

---

## 1. Concept & Architecture of Spatial Results

`Spatial Result` is the platform's standardized domain data model for representing geospatial analytic outputs produced by drone image processing plugins (such as vegetation index calculation, land-cover classification, individual tree canopy detection, and inspection routes) on the interactive Leaflet map interface.

### Pipeline Role

```text
┌────────────────────────┐      ┌──────────────────────────┐      ┌─────────────────────────┐
│ Analytic Plugin        │─────▶│ Rust Backend (Module 03) │─────▶│ Leaflet Frontend / UI   │
│ (Python / WASM / Rust) │      │ map_storage/ & IPC Handl │      │ Vector Rendering        │
│ Output: Result GeoJSON │      │ Atomic IO, Validation    │      │ Inspection Popups & UI  │
└────────────────────────┘      └──────────────────────────┘      └─────────────────────────┘
```

1. **Unified Bridge**: Standardizes multi-plugin outputs into an RFC 7946 GeoJSON-compliant `LayerPayload`.
2. **Dual-Storage Engine**: Persisted in `map_storage/spatial_results/` either as a standalone flat file (`.json`) or a decomposed feature directory (`features/{feature_id}.json`).
3. **Synchronized Layer Registration**: Automatically syncs layer display metadata in `map_storage/layers/{layer_id}.json`.

---

## 2. JSON Schema & Authoring Guidelines

Every `Spatial Result` file must conform to the `LayerPayload` structure.

### 2.1. Top-Level Structure (`LayerPayload`)

```json
{
  "layer_id": "result_vegetation_001",
  "layer_name": "Vegetation Detection (ExG) - Palm Block A",
  "display_preference": { ... },
  "spatial_results": [ ... ],
  "updated_at": "2026-10-26T05:32:18.847Z",
  "concurrency_token": "2026-10-26T05:32:18.847Z"
}
```

| Field                | Type   | Required | Constraints & Notes                                                                                                                                   |
| :------------------- | :----- | :------: | :---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `layer_id`           | String | **Yes**  | Unique identifier. **Regex Constraint**: `^[A-Za-z0-9_-]{1,64}$`. Must not contain whitespace, slashes (`/`, `\`), or `..` (prevents path traversal). |
| `layer_name`         | String | **Yes**  | Human-readable layer display title (1–100 characters, cannot be whitespace-only).                                                                     |
| `display_preference` | Object | **Yes**  | Map styling configuration (Polygon, Point, Line, or ImageOverlay).                                                                                    |
| `spatial_results`    | Array  | **Yes**  | Array of spatial features (`SpatialResult`) containing geometry and attributes.                                                                       |
| `updated_at`         | String | Optional | ISO 8601 UTC timestamp of creation or latest mutation.                                                                                                |
| `concurrency_token`  | String | Optional | Optimistic locking token to guard against lost updates.                                                                                               |

---

### 2.2. Display Preference Schema (`display_preference`)

Four visualization modes are supported using a tagged union (`"type"`):

#### A. Polygon Mode (`"type": "polygon"`)

Used for vegetation segmentation, land cover areas, or boundary AOIs.

```json
{
  "type": "polygon",
  "color": "#0074d9",
  "opacity": 0.6,
  "image_url": "/map_storage/images/result_image/result_vegetation_001_1792992738847.jpg",
  "fields": [
    {
      "key": "vegetation_index_type",
      "label": "Index Type",
      "visible": true,
      "custom": false,
      "kind": "text"
    },
    {
      "key": "mean_greenness_score",
      "label": "Mean Score",
      "visible": true,
      "custom": false,
      "kind": "number"
    },
    {
      "key": "area_sqm",
      "label": "Area",
      "unit": "m²",
      "visible": true,
      "custom": false,
      "kind": "number"
    }
  ]
}
```

#### B. Point Mode (`"type": "point"`)

Used for individual tree detection, canopy centers, or point anomalies.

```json
{
  "type": "point",
  "color": "#2ecc40",
  "icon": "tree",
  "fields": [
    { "key": "tree_id", "label": "Tree ID", "visible": true, "kind": "text" },
    { "key": "confidence", "label": "Confidence", "visible": true, "kind": "number" },
    {
      "key": "height_est_m",
      "label": "Estimated Height",
      "unit": "m",
      "visible": true,
      "kind": "number"
    }
  ]
}
```

#### C. Line Mode (`"type": "line"`)

Used for flight paths, drainage channels, or inspection transects.

```json
{
  "type": "line",
  "color": "#ff851b",
  "weight": 3.0,
  "fields": [
    { "key": "path_id", "label": "Path ID", "visible": true, "kind": "text" },
    { "key": "length_m", "label": "Length", "unit": "m", "visible": true, "kind": "number" }
  ]
}
```

#### D. Image Overlay Mode (`"type": "image_overlay"`)

Used for raster heatmaps or georeferenced analytical masks.

```json
{
  "type": "image_overlay",
  "image_url": "/map_storage/runs/001/output_mask_01.png",
  "opacity": 0.75,
  "bounds": [
    [-1.2228, 116.857],
    [-1.2208, 116.8592]
  ]
}
```

_Note: `bounds` format is `[[South, West], [North, East]]` in Latitude / Longitude._

---

### 2.3. Presentation Field Definitions (`FieldDef`)

Each item in the `fields` array dictates how attribute keys appear in the Leaflet popup and data table:

- `key`: Property key matching the feature's `properties` map.
- `label`: Human-readable column header or popup label.
- `unit`: Optional metric unit (e.g. `m²`, `ha`, `%`, `m`).
- `visible`: Visibility flag (`true` or `false`).
- `custom`: Flag indicating if the field was manually appended by the user in the UI editor.
- `kind`: Primitive type hint (`"text"`, `"number"`, `"boolean"`).

---

### 2.4. Individual Feature Structure (`SpatialResult`)

Each element inside `spatial_results` represents a distinct geographical feature:

```json
{
  "source": "vegetation_detection_v1",
  "geometry": {
    "type": "Polygon",
    "coordinates": [
      [
        [116.8575, -1.221],
        [116.8582, -1.2208],
        [116.859, -1.2212],
        [116.8575, -1.221]
      ]
    ]
  },
  "properties": {
    "vegetation_index_type": "ExG",
    "mean_greenness_score": 0.68,
    "area_sqm": 1420.5
  },
  "image_url": null,
  "edited_fields": []
}
```

1. **`source`**: Originating algorithm or plugin identifier.
2. **`geometry`**: RFC 7946 GeoJSON geometry:
   - Coordinate Reference System: **WGS 84 (EPSG:4326)**.
   - Coordinate Tuple Order: **`[Longitude, Latitude]`** (NOT `[lat, lon]`).
   - Supported Types: `Point`, `Polygon`, or `LineString`.
3. **`properties`**: Strongly-typed or polymorphic attributes:
   - `TreeProperties`: `tree_id`, `confidence`, `height_est_m`.
   - `VegetationProperties`: `vegetation_index_type`, `mean_greenness_score`, `area_sqm`.
   - `LandCoverProperties`: `land_cover_class`, `confidence`, `area_sqm`.
   - `InspectionPathProperties`: `path_id`, `length_m`.
   - `Generic`: Arbitrary key-value map for experimental or custom plugin attributes.
4. **`image_url`**: Optional direct image asset associated with the feature.
5. **`edited_fields`**: List of property keys modified manually by the user.

---

## 3. Storage Architecture & Backend Mechanics

### 3.1. Dual-Storage Support

The backend reader (`src-tauri/src/modules/module_03/sp_result/reader.rs`) supports two storage structures:

1. **Flat File Mode (`map_storage/spatial_results/{layer_id}.json`)**:
   - Single consolidated file.
   - Preferred for small to medium datasets (< 5,000 features).
2. **Decomposed Directory Mode (`map_storage/spatial_results/{layer_id}/`)**:
   - Designed for massive spatial datasets to minimize memory spikes:
     - `layer.json`: Layer metadata and styling configuration (enforced <= 64 KB).
     - `features/{feature_id}.json`: Atomically written individual feature files (enforced <= 64 KB per feature).

### 3.2. Atomic Write-Through Guarantee

All writes utilize `write_atomic`:

1. JSON serialized and written to temporary file `temp_{uuid}.tmp`.
2. Flushed to underlying OS storage buffers.
3. Atomically renamed to target destination.
4. **Guarantee**: Eliminates file truncation and corruption during unexpected power cuts or OS crashes.

### 3.3. Layer State Synchronization (`map_storage/layers/`)

Calling `save_spatial_result` automatically generates or updates `map_storage/layers/{layer_id}.json` with `LayerState`:

```json
{
  "layer_id": "result_vegetation_001",
  "layer_name": "Vegetation Detection (ExG) - Palm Block A",
  "category": "polygon",
  "is_visible": true,
  "opacity": 0.6,
  "z_index": 10,
  "source_type": "spatial_result",
  "source_id": "result_vegetation_001"
}
```

This guarantees the layer immediately reflects in the Leaflet Layer Control tree without requiring frontend manual syncing.

---

## 4. Lifecycle & Mutation Workflows

### 4.1. Tauri IPC Commands

| Command                             | Access | Description                                                                                  |
| :---------------------------------- | :----: | :------------------------------------------------------------------------------------------- |
| `save_spatial_result`               | Write  | Persists a complete layer payload atomically. Emits `spatial_result:saved`.                  |
| `get_saved_spatial_results`         |  Read  | Reads all saved spatial results with complete GeoJSON geometry.                              |
| `get_saved_spatial_results_summary` |  Read  | Returns lightweight `SpatialResultSummary` objects for high-performance card/list rendering. |
| `get_saved_spatial_result`          |  Read  | Retrieves a single complete spatial result by `layer_id` via dual-reader.                    |
| `update_saved_spatial_result`       | Write  | Performs non-destructive semantic patching on presentation attributes.                       |
| `count_spatial_result_dependents`   |  Read  | Pre-computes dependent files and photos for safe deletion dialogs.                           |
| `delete_saved_spatial_result`       | Delete | Deletes a layer using 2-phase transactional quarantine. Emits `spatial_result:deleted`.      |
| `check_spatial_result_manual_edits` |  Read  | Checks if a layer contains user-edited features before plugin overwrite.                     |

---

### 4.2. Semantic Patching & Manual Edits Audit

The `update_saved_spatial_result` command enables safe modifications:

- **Presentation Updates**: Layer title, hex color (`#RRGGBB`), and icon.
- **Image Lifecycle**: Secure photo upload/replacement (`keep`, `delete`, `replace`) stored under `map_storage/images/result_image/{layer_id}_{timestamp}.{ext}` after magic byte validation (JPEG/PNG/WebP, max 10 MB).
- **Edit Tracking**: Modified property fields are appended to `edited_fields`, protecting user modifications from automated plugin overrides.
- **Optimistic Concurrency**: Guards against conflicting writes via `expected_updated_at`.

---

### 4.3. 2-Phase Quarantine & Transactional Rollback

Deletion enforces maximum safety:

1. **Phase 1 (Quarantine)**: Files and associated photos are moved to `.trash_sr_{layer_id}_{timestamp}/`.
2. **Phase 2 (Unregistration)**: Layer registration in `map_storage/layers/{layer_id}.json` is removed.
3. **Rollback**: If unregistration encounters an IO error, quarantined files are automatically restored.
4. **Startup Reconciliation**: `cleanup_orphaned_trash` scans on startup to either restore orphaned data or finalize incomplete purges.

---

## 5. Concrete Ready-to-Use JSON Example

### Example: Individual Tree Detection (Point Layer)

```json
{
  "layer_id": "result_tree_001",
  "layer_name": "Palm Tree Census Block B12",
  "display_preference": {
    "type": "point",
    "color": "#2ecc40",
    "icon": "tree",
    "fields": [
      {
        "key": "tree_id",
        "label": "Tree Number",
        "visible": true,
        "custom": false,
        "kind": "text"
      },
      {
        "key": "confidence",
        "label": "Confidence Level",
        "visible": true,
        "custom": false,
        "kind": "number"
      },
      {
        "key": "height_est_m",
        "label": "Estimated Height",
        "unit": "m",
        "visible": true,
        "custom": false,
        "kind": "number"
      }
    ]
  },
  "spatial_results": [
    {
      "source": "tree_detector_yolov8",
      "geometry": {
        "type": "Point",
        "coordinates": [116.8582, -1.2208]
      },
      "properties": {
        "tree_id": "TREE_001",
        "confidence": 0.94,
        "height_est_m": 8.5
      }
    },
    {
      "source": "tree_detector_yolov8",
      "geometry": {
        "type": "Point",
        "coordinates": [116.8585, -1.2211]
      },
      "properties": {
        "tree_id": "TREE_002",
        "confidence": 0.91,
        "height_est_m": 7.8
      }
    }
  ],
  "updated_at": "2026-10-26T08:00:00.000Z"
}
```

---

## 6. Developer Checklist for Plugin Authors

When outputting spatial results from an analytical plugin:

1. Ensure coordinate arrays are **`[Longitude, Latitude]`** (EPSG:4326).
2. Set a unique, sanitized `layer_id` matching `^[A-Za-z0-9_-]{1,64}$`.
3. Provide descriptive `fields` in `display_preference` with appropriate `unit` and `kind`.
4. Wrap output in `LayerPayload` format before invoking `save_spatial_result`.
