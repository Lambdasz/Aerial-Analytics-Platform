# Frontend Pipelines & Lifecycle: Module 03

This document details the reactive pipelines, data flow lifecycles, and layer resolution sequences in the **Module 03 (Aerial Image Map Explorer)** frontend.

---

## 1. User Interaction to Backend IPC Pipeline

Every user-initiated mutation follows a strict unidirectional data flow to preserve state consistency and error isolation:

```text
+-------------------+      User Event      +--------------------+
|  UI Molecule /    | -------------------> |  React Organism /  |
|  Control Toolbar  |                      |  Custom Hook State |
+-------------------+                      +--------------------+
                                                     |
                                                     | Validates Draft &
                                                     | Sets Loading State
                                                     v
+-------------------+     invokeMap()      +--------------------+
|   Tauri 2 Core    | <------------------- |  services/api.ts   |
|   Rust Backend    |                      |  (Typed IPC Call)  |
+-------------------+                      +--------------------+
         |
         | Result / CommandError
         v
+-------------------+    State Commit /    +--------------------+
|   Local Cache /   | -------------------> | UI Feedback Toast /|
|  Hook Synchronizer|    Error Recovery    | Leaflet Re-render  |
+-------------------+                      +--------------------+
```

### Flow Stages

1. **User Interaction**: An action is triggered via a UI molecule (e.g. clicking "Save AOI" in `AoiCreateForm` or toggling opacity in `LayerManager`).
2. **Hook Validation**: The governing organism or custom hook (`useAOI`, `useSpatialResultLayers`) validates parameters and sets optimistic or pending status.
3. **Transport Layer**: The typed function in `services/api.ts` invokes Tauri IPC via `invokeMap`.
4. **Result Commitment**: On success, internal state updates and child components re-render. On failure, errors are mapped to user-friendly Blueprint toaster notices.

---

## 2. Spatial Result Layer Rendering Pipeline

Spatial analysis results (vegetation clusters, landcover segmentation) are ingested, formatted, and converted into Leaflet overlays:

```text
[ Disk Storage: map_storage/spatial_results/ ]
                       |
                       | IPC: get_saved_spatial_results()
                       v
        [ Hook: useSpatialResultLayers ]
                       |
                       | Transform & Normalize
                       v
        [ Pure: spatialLayerBuilder.ts ]
          - Parse GeoJSON Polygons / Points
          - Inject Dynamic Styles & Fill Colors
          - Generate Tooltip & Popup HTML
                       |
                       v
        [ Organism: ManagedMapLayers.tsx ]
          - Declarative <GeoJSON> & <Polygon>
          - Leaflet Map Canvas Injection
```

---

## 3. Image URL Resolution Pipeline (`resolveImageSrc`)

Resolving image URLs (orthomosaic thumbnails, detection photo crops) requires handling three distinct environments: external web protocols, local Vite development, and packaged Tauri desktop binaries.

```text
Raw Image URL String
         |
         +---> 1. Starts with "http://", "https://", "data:", or "asset:"?
         |        YES -> Return trimmed URL directly.
         |
         +---> 2. Contains path traversal ("..")?
         |        YES -> Log security warning & Return null.
         |
         +---> 3. Contains "map_storage/"?
         |        |
         |        +---> Environment: Tauri Desktop Runtime (isTauri() == true)?
         |        |     YES -> Fetch cached storageRoot (get_storage_root_path)
         |        |            Join canonical path + relative asset path
         |        |            Pass to convertFileSrc() -> "asset://localhost/..."
         |        |
         |        +---> Environment: Web Browser Dev Server (isTauri() == false)?
         |              YES -> Serve via Vite middleware -> "/map_storage/..."
         |
         +---> 4. Static Public Asset (e.g. "/sample_drone.jpg")?
                  Return path with leading slash.
```

---

## 4. AOI Drawing & Calculation Pipeline

```text
[ User Selects Polygon Tool on DrawingControlBar ]
                       |
                       v
[ Leaflet-Geoman Event: pm:create ]
                       |
                       v
[ Hook: useLeafletGeoman ]
  - Intercepts Layer Creation
  - Extracts Coordinate Rings: [[lat, lng], ...]
                       |
                       v
[ Pure: aoiCalculations.ts ]
  - Computes Shoelace Area (m² and Hectares)
  - Computes Geodesic Haversine Perimeter (m)
  - Computes Bounding Box (SouthWest, NorthEast)
                       |
                       v
[ Molecule: AoiCreateForm.tsx ]
  - User Names AOI and Picks Display Color
                       |
                       v
[ API: saveAoi() via invokeMap ]
  - Backend writes atomically to map_storage/aois/{id}.json
```
