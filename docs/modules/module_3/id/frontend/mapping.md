# Pemetaan API Frontend: Modul 03

Dokumen ini memetakan seluruh 38 fungsi transportasi API yang diekspor dan didefinisikan dalam [`src/modules/module-03/services/api.ts`](file:///d:/project/Aerial-Analytics-Platform/src/modules/module-03/services/api.ts) ke perintah Tauri IPC dasarnya, komponen/hook pemanggil, dan antarmuka payload.

---

## 1. Overview & Transport Contract

Seluruh fungsi API frontend membungkus utilitas transportasi IPC terstandarisasi `invokeMap`:

- **Berkas**: `src/modules/module-03/services/api.ts`
- **Normalisasi Error**: Memetakan kegagalan backend menjadi instans `CommandError`.
- **Fallback Tiruan**: Dalam pengembangan peramban mandiri (non-Tauri), `invokeMap` mencegat perintah dan menyediakan data fallback fixture.

---

## 2. Complete Frontend API Mapping Table (38 Methods)

| API Function Name               | Underlying Tauri IPC Command        | Primary Calling Hooks & Components       | Request Type                          | Response Type                     | Error Handling & Fallback                            |
| :------------------------------ | :---------------------------------- | :--------------------------------------- | :------------------------------------ | :-------------------------------- | :--------------------------------------------------- |
| `measureDistance`               | `measure_distance`                  | `MapViewControlBar`                      | `points: number[][]`                  | `number`                          | Mengembalikan jarak Euclidean/geodesik dalam meter   |
| `measureArea`                   | `measure_area`                      | `MapViewControlBar`                      | `polygon: number[][]`                 | `number`                          | Mengembalikan luas Shoelace dalam meter persegi      |
| `measurePerimeter`              | `measure_perimeter`                 | `MapViewControlBar`                      | `polygon: number[][]`                 | `number`                          | Mengembalikan keliling dalam meter                   |
| `saveNewAoi`                    | `save_new_aoi_cmd`                  | `useAOI`, `AoiCreateForm`                | `payload: SaveAoiPayload`             | `AoiFeature`                      | Membuat dan menyimpan AOI baru beserta metrik        |
| `createAoi`                     | `create_aoi_cmd`                    | `useAOI`                                 | `polygon: AoiPolygon`                 | `AoiFeature`                      | Memvalidasi koordinat poligon                        |
| `saveAoi`                       | `save_aoi`                          | `useAOI`                                 | `aoi: AoiFeature`                     | `AoiFeature`                      | Menyimpan fitur AOI yang sudah terkonstruksi ke disk |
| `getSavedAois`                  | `get_saved_aois`                    | `useAOI`                                 | `void`                                | `AoiFeature[]`                    | Menormalisasi menjadi `[]` jika terjadi error        |
| `deleteSavedAoi`                | `delete_saved_aoi`                  | `useAOI`, `DeleteAoiDialog`              | `id: string`                          | `boolean`                         | Melempar `CommandError` saat gagal                   |
| `saveSpatialResult`             | `save_spatial_result`               | Plugin AI, Ingest                        | `result: SpatialResultItem`           | `SpatialResultItem`               | Menyimpan hasil ke `spatial_results/`                |
| `getSavedSpatialResults`        | `get_saved_spatial_results`         | `useSpatialResultLayers`                 | `void`                                | `SpatialResultItem[]`             | Memuat pohon hasil spasial lengkap                   |
| `deleteSavedSpatialResult`      | `delete_saved_spatial_result`       | `DeleteConfirmDialog`                    | `id: string`                          | `boolean`                         | Menghapus direktori atau berkas tunggal              |
| `createAnnotation`              | `create_annotation_cmd`             | `useAnnotations`, `LayerManager`         | `payload: CreateAnnotationPayload`    | `AnnotationFeature`               | Memvalidasi batas ukuran JSON 64 KB                  |
| `getSavedAnnotations`           | `get_saved_annotations`             | `useAnnotations`                         | `void`                                | `AnnotationFeature[]`             | Mengembalikan array kosong jika error                |
| `getAnnotationById`             | `get_annotation_by_id`              | `useAnnotations`                         | `id: string`                          | `AnnotationFeature`               | Melempar `NOT_FOUND` jika data tidak ada             |
| `updateAnnotation`              | `update_annotation_cmd`             | `useAnnotations`                         | `annotation: AnnotationFeature`       | `AnnotationFeature`               | Menggantikan anotasi yang sudah ada                  |
| `deleteAnnotation`              | `delete_annotation_cmd`             | `useAnnotations`, `LayerManager`         | `id: string`                          | `boolean`                         | Menghapus dari disk dan cache memori                 |
| `getProjectImageMarkers`        | `get_photos`                        | `useImageMarkers`, `ImageGallerySidebar` | `projectId?: string`                  | `DroneImageMetadata[]`            | Mengembalikan penanda foto drone                     |
| `getInstanceStatus`             | `get_instance_status`               | Shell Aplikasi, Header                   | `void`                                | `InstanceStatus`                  | Mengembalikan apakah proses merupakan instans utama  |
| `getInitialMapLayers`           | `get_saved_layers`                  | `LayerManager`                           | `void`                                | `LayerState[]`                    | Mengembalikan daftar layer peta terdaftar            |
| `saveImageFromPath`             | `save_image_from_path`              | Fallback legacy                          | `path: string, category?: string`     | `StoredImageMetadata`             | Stub usang yang mengarahkan ke impor job             |
| `getSavedImages`                | `get_saved_images`                  | Fallback legacy                          | `void`                                | `StoredImageMetadata[]`           | Perintah legacy daftar citra                         |
| `deleteSavedImage`              | `delete_saved_image`                | Fallback legacy                          | `category: string, filename: string`  | `boolean`                         | Perintah legacy penghapusan citra                    |
| `importOrthoImage`              | `import_ortho_image`                | Fallback legacy                          | `path: string, category?: string`     | `StoredImageMetadata`             | Perintah impor sinkron legacy                        |
| `listOrthoImages`               | `list_ortho_images`                 | `OrthoImportPanel`, `useAOI`             | `void`                                | `StoredImageMetadata[]`           | Mendaftar semua citra ortomosaik tersimpan           |
| `getOrthoImage`                 | `get_ortho_image`                   | `OrthoDetailModal`                       | `id: string`                          | `StoredImageMetadata`             | Mengambil rincian citra ortomosaik                   |
| `updateOrthoImage`              | `update_ortho_image`                | `OrthoEditModal`                         | `payload: UpdateOrthoImagePayload`    | `StoredImageMetadata`             | Memperbarui nama atau batas koordinat                |
| `getOrthoCascadeSummary`        | `get_ortho_cascade_summary`         | `OrthoDeleteModal`                       | `id: string`                          | `DeleteOrthoSummary`              | Menghitung layer dependen sebelum dihapus            |
| `deleteOrthoImage`              | `delete_ortho_image`                | `OrthoDeleteModal`                       | `id: string`                          | `DeleteOrthoSummary`              | Menjalankan penghapusan cascade lengkap              |
| `startImportOrthoJob`           | `start_import_ortho_job`            | `OrthoUploadModal`, `OrthoImportPanel`   | `payload: ImportDualModePayload`      | `string`                          | Menerapkan kunci instans utama                       |
| `cancelImportJob`               | `cancel_import_job`                 | `OrthoImportPanel`                       | `jobId: string`                       | `void`                            | Mengubah status job menjadi dibatalkan               |
| `getImportJobStatus`            | `get_import_job_status`             | `OrthoImportPanel`                       | `jobId: string`                       | `JobRecord`                       | Polling progres dari `.jobs/{id}.json`               |
| `inspectGeotiffHeader`          | `inspect_geotiff_header`            | `OrthoUploadModal`                       | `filePath: string`                    | `GeotiffInspectionResult`         | Pra-validasi cepat header GeoTIFF                    |
| `getSavedSpatialResultsSummary` | `get_saved_spatial_results_summary` | `SpatialResultTree`                      | `void`                                | `SpatialResultSummaryItem[]`      | Ringkasan header hasil yang ringan                   |
| `getSavedSpatialResult`         | `get_saved_spatial_result`          | `SpatialResultDetail`                    | `id: string`                          | `SpatialResultItem`               | Memuat satu hasil spasial terperinci                 |
| `updateSavedSpatialResult`      | `update_saved_spatial_result`       | `EditSpResultModal`                      | `payload: UpdateSpatialResultPayload` | `SpatialResultItem`               | Tambal properti & sinkronisasi foto                  |
| `countSpatialResultDependents`  | `count_spatial_result_dependents`   | `DeleteConfirmDialog`                    | `id: string`                          | `SpatialResultDependentsSummary`  | Menginspeksi jumlah layer dependen                   |
| `checkSpatialResultManualEdits` | `check_spatial_result_manual_edits` | `DeleteConfirmDialog`                    | `id: string`                          | `SpatialResultManualEditsSummary` | Menginspeksi apakah fitur memiliki editan manual     |
| `getStorageRootPath`            | `get_storage_root_path`             | `fieldResolution.ts`                     | `void`                                | `string`                          | Path kanonikal penyimpanan untuk aset                |
