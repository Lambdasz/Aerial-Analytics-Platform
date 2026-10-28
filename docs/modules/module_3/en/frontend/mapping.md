# Frontend API Mapping: Module 03

This document maps all 38 exported API transport functions defined in [`src/modules/module-03/services/api.ts`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/services/api.ts) to their underlying Tauri IPC commands, caller components/hooks, and payload interfaces.

---

## 1. Overview & Transport Contract

All frontend API functions wrap the standardized IPC transport utility `invokeMap`:

- **File**: `src/modules/module-03/services/api.ts`
- **Error Normalization**: Maps backend failures into `CommandError` instances.
- **Mock Fallback**: In standalone browser development (non-Tauri), `invokeMap` intercepts commands and provides fixture fallbacks.

---

## 2. Complete Frontend API Mapping Table (38 Methods)

| API Function Name               | Underlying Tauri IPC Command        | Primary Calling Hooks & Components       | Request Type                          | Response Type                     | Error Handling & Fallback                     |
| :------------------------------ | :---------------------------------- | :--------------------------------------- | :------------------------------------ | :-------------------------------- | :-------------------------------------------- |
| `measureDistance`               | `measure_distance`                  | `MapViewControlBar`                      | `points: number[][]`                  | `number`                          | Returns Euclidean/geodesic distance in meters |
| `measureArea`                   | `measure_area`                      | `MapViewControlBar`                      | `polygon: number[][]`                 | `number`                          | Returns Shoelace area in square meters        |
| `measurePerimeter`              | `measure_perimeter`                 | `MapViewControlBar`                      | `polygon: number[][]`                 | `number`                          | Returns perimeter in meters                   |
| `saveNewAoi`                    | `save_new_aoi_cmd`                  | `useAOI`, `AoiCreateForm`                | `payload: SaveAoiPayload`             | `AoiFeature`                      | Creates and persists new AOI with metrics     |
| `createAoi`                     | `create_aoi_cmd`                    | `useAOI`                                 | `polygon: AoiPolygon`                 | `AoiFeature`                      | Validates polygon coordinates                 |
| `saveAoi`                       | `save_aoi`                          | `useAOI`                                 | `aoi: AoiFeature`                     | `AoiFeature`                      | Persists pre-constructed AOI feature to disk  |
| `getSavedAois`                  | `get_saved_aois`                    | `useAOI`                                 | `void`                                | `AoiFeature[]`                    | Normalizes to `[]` on error                   |
| `deleteSavedAoi`                | `delete_saved_aoi`                  | `useAOI`, `DeleteAoiDialog`              | `id: string`                          | `boolean`                         | Throws `CommandError` on failure              |
| `saveSpatialResult`             | `save_spatial_result`               | AI Plugins, Ingest                       | `result: SpatialResultItem`           | `SpatialResultItem`               | Persists result to `spatial_results/`         |
| `getSavedSpatialResults`        | `get_saved_spatial_results`         | `useSpatialResultLayers`                 | `void`                                | `SpatialResultItem[]`             | Loads full spatial result trees               |
| `deleteSavedSpatialResult`      | `delete_saved_spatial_result`       | `DeleteConfirmDialog`                    | `id: string`                          | `boolean`                         | Deletes directory or single JSON              |
| `createAnnotation`              | `create_annotation_cmd`             | `useAnnotations`, `LayerManager`         | `payload: CreateAnnotationPayload`    | `AnnotationFeature`               | Validates 64 KB JSON size ceiling             |
| `getSavedAnnotations`           | `get_saved_annotations`             | `useAnnotations`                         | `void`                                | `AnnotationFeature[]`             | Returns empty array on error                  |
| `getAnnotationById`             | `get_annotation_by_id`              | `useAnnotations`                         | `id: string`                          | `AnnotationFeature`               | Throws `NOT_FOUND` if missing                 |
| `updateAnnotation`              | `update_annotation_cmd`             | `useAnnotations`                         | `annotation: AnnotationFeature`       | `AnnotationFeature`               | Replaces existing annotation                  |
| `deleteAnnotation`              | `delete_annotation_cmd`             | `useAnnotations`, `LayerManager`         | `id: string`                          | `boolean`                         | Removes from disk and memory cache            |
| `getProjectImageMarkers`        | `get_photos`                        | `useImageMarkers`, `ImageGallerySidebar` | `projectId?: string`                  | `DroneImageMetadata[]`            | Returns drone photo markers                   |
| `getInstanceStatus`             | `get_instance_status`               | App Shell, Header                        | `void`                                | `InstanceStatus`                  | Returns whether process is primary            |
| `getInitialMapLayers`           | `get_saved_layers`                  | `LayerManager`                           | `void`                                | `LayerState[]`                    | Returns registered map layers                 |
| `saveImageFromPath`             | `save_image_from_path`              | Legacy fallback                          | `path: string, category?: string`     | `StoredImageMetadata`             | Deprecated stub directing to job import       |
| `getSavedImages`                | `get_saved_images`                  | Legacy fallback                          | `void`                                | `StoredImageMetadata[]`           | Legacy image list command                     |
| `deleteSavedImage`              | `delete_saved_image`                | Legacy fallback                          | `category: string, filename: string`  | `boolean`                         | Legacy deletion command                       |
| `importOrthoImage`              | `import_ortho_image`                | Legacy fallback                          | `path: string, category?: string`     | `StoredImageMetadata`             | Legacy sync import command                    |
| `listOrthoImages`               | `list_ortho_images`                 | `OrthoImportPanel`, `useAOI`             | `void`                                | `StoredImageMetadata[]`           | Lists all stored orthomosaics                 |
| `getOrthoImage`                 | `get_ortho_image`                   | `OrthoDetailModal`                       | `id: string`                          | `StoredImageMetadata`             | Fetches orthomosaic details                   |
| `updateOrthoImage`              | `update_ortho_image`                | `OrthoEditModal`                         | `payload: UpdateOrthoImagePayload`    | `StoredImageMetadata`             | Updates name or bounds                        |
| `getOrthoCascadeSummary`        | `get_ortho_cascade_summary`         | `OrthoDeleteModal`                       | `id: string`                          | `DeleteOrthoSummary`              | Counts dependent layers before delete         |
| `deleteOrthoImage`              | `delete_ortho_image`                | `OrthoDeleteModal`                       | `id: string`                          | `DeleteOrthoSummary`              | Performs complete cascade deletion            |
| `startImportOrthoJob`           | `start_import_ortho_job`            | `OrthoUploadModal`, `OrthoImportPanel`   | `payload: ImportDualModePayload`      | `string`                          | Enforces primary instance lock                |
| `cancelImportJob`               | `cancel_import_job`                 | `OrthoImportPanel`                       | `jobId: string`                       | `void`                            | Sets job status to cancelled                  |
| `getImportJobStatus`            | `get_import_job_status`             | `OrthoImportPanel`                       | `jobId: string`                       | `JobRecord`                       | Polls progress from `.jobs/{id}.json`         |
| `inspectGeotiffHeader`          | `inspect_geotiff_header`            | `OrthoUploadModal`                       | `filePath: string`                    | `GeotiffInspectionResult`         | Fast pre-validation of GeoTIFF                |
| `getSavedSpatialResultsSummary` | `get_saved_spatial_results_summary` | `SpatialResultTree`                      | `void`                                | `SpatialResultSummaryItem[]`      | Fast lightweight result headers               |
| `getSavedSpatialResult`         | `get_saved_spatial_result`          | `SpatialResultDetail`                    | `id: string`                          | `SpatialResultItem`               | Loads single detailed result                  |
| `updateSavedSpatialResult`      | `update_saved_spatial_result`       | `EditSpResultModal`                      | `payload: UpdateSpatialResultPayload` | `SpatialResultItem`               | In-place property & photo sync                |
| `countSpatialResultDependents`  | `count_spatial_result_dependents`   | `DeleteConfirmDialog`                    | `id: string`                          | `SpatialResultDependentsSummary`  | Inspects dependent layers count               |
| `checkSpatialResultManualEdits` | `check_spatial_result_manual_edits` | `DeleteConfirmDialog`                    | `id: string`                          | `SpatialResultManualEditsSummary` | Inspects whether features have manual edits   |
| `getStorageRootPath`            | `get_storage_root_path`             | `fieldResolution.ts`                     | `void`                                | `string`                          | Canonical storage path for assets             |
