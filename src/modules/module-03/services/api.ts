/**
 * Typed wrappers around Tauri IPC commands for AOI & spatial measurement.
 *
 * All calls go through {@link invokeMap} (from `error.ts`) so error logging
 * and structured error handling are centralised.
 *
 * @module
 */

import { invokeMap } from "./error";
import type { AoiFeature, Coordinate, Polygon } from "../types/aoi";
import type { AnnotationFeature, LayerState } from "../types/layer";
import type { DroneImageMetadata } from "../types/marker";

import type { LayerPayload } from "../types/map";
import type {
  DeleteOrthoSummary,
  GeotiffInspectionResult,
  ImportDualModeParams,
  ImportImageParams,
  JobRecord,
  StoredImageMetadata,
  UpdateOrthoImageParams,
} from "../types/storage";
import type {
  DeleteSpatialResultSummary,
  ManualEditsCheckResult,
  SpatialResultSummary,
  UpdateSpatialResultPayload,
} from "../types/spatialResult";

/**
 * Calculates the great-circle distance between two points (Haversine).
 *
 * @param pointA — `[longitude, latitude]` of the first point.
 * @param pointB — `[longitude, latitude]` of the second point.
 * @returns Distance in **meters**.
 */
export function measureDistance(pointA: Coordinate, pointB: Coordinate): Promise<number> {
  return invokeMap<number>("measure_distance", { pointA, pointB });
}

/**
 * Calculates the area of a polygon (spherical excess, WGS84).
 *
 * @param polygon — GeoJSON-style polygon rings.
 * @returns Area in **square meters**.
 */
export function measureArea(polygon: Polygon): Promise<number> {
  return invokeMap<number>("measure_area", { polygon });
}

/**
 * Calculates the perimeter of a polygon.
 *
 * @param polygon — GeoJSON-style polygon rings.
 * @returns Perimeter in **meters**.
 */
export function measurePerimeter(polygon: Polygon): Promise<number> {
  return invokeMap<number>("measure_perimeter", { polygon });
}

/**
 * Creates and persists an AOI GeoJSON Feature atomically in a single IPC call.
 */
export function saveNewAoi(args: {
  aoiId: string;
  name: string;
  aoiType?: string;
  polygon: Polygon;
  createdAt?: string;
}): Promise<AoiFeature> {
  return invokeMap<AoiFeature>("save_new_aoi_cmd", args);
}

/**
 * Creates a validated AOI GeoJSON Feature.
 *
 * The backend validates the polygon, computes area & perimeter, and
 * assembles the complete {@link AoiFeature}.
 */
export function createAoi(args: {
  aoiId: string;
  name: string;
  aoiType?: string;
  polygon: Polygon;
  createdAt: string;
}): Promise<AoiFeature> {
  return invokeMap<AoiFeature>("create_aoi_cmd", args);
}

/**
 * Persists an AOI GeoJSON Feature to the backend `/storage/aois/{aoi_id}.json`.
 */
export function saveAoi(aoi: AoiFeature): Promise<AoiFeature> {
  return invokeMap<AoiFeature>("save_aoi", { aoi });
}

/**
 * Retrieves all saved AOIs from `/storage/aois/`.
 */
export function getSavedAois(): Promise<AoiFeature[]> {
  return invokeMap<AoiFeature[]>("get_saved_aois");
}

/**
 * Deletes a saved AOI from storage by its ID.
 */
export function deleteSavedAoi(aoiId: string): Promise<boolean> {
  return invokeMap<boolean>("delete_saved_aoi", { aoiId, aoi_id: aoiId });
}

/**
 * Persists a spatial result LayerPayload to `/storage/spatial_results/{layer_id}.json`.
 */
export function saveSpatialResult(payload: LayerPayload): Promise<LayerPayload> {
  return invokeMap<LayerPayload>("save_spatial_result", { payload });
}

/**
 * Retrieves all saved spatial results from `/storage/spatial_results/`.
 */
export function getSavedSpatialResults(): Promise<LayerPayload[]> {
  return invokeMap<LayerPayload[]>("get_saved_spatial_results");
}

/**
 * Deletes a saved spatial result from storage by its layer ID.
 */
export function deleteSavedSpatialResult(layerId: string): Promise<boolean> {
  return invokeMap<boolean>("delete_saved_spatial_result", { layerId });
}

export function createAnnotation(args: {
  annotationId?: string;
  id?: string;
  text: string;
  geometry: AnnotationFeature["geometry"];
  createdAt: string;
}): Promise<AnnotationFeature> {
  const finalId = args.id || args.annotationId || "";
  return invokeMap<AnnotationFeature>("create_annotation_cmd", {
    annotationId: finalId,
    text: args.text,
    geometry: args.geometry,
    createdAt: args.createdAt,
  });
}

/**
 * Retrieves all saved annotations from `/map_storage/annotations/`.
 */
export function getSavedAnnotations(): Promise<AnnotationFeature[]> {
  return invokeMap<AnnotationFeature[]>("get_saved_annotations");
}

/**
 * Retrieves a single annotation by ID from `/map_storage/annotations/`.
 */
export function getAnnotationById(annotationId: string): Promise<AnnotationFeature> {
  return invokeMap<AnnotationFeature>("get_annotation_by_id", {
    annotationId,
    annotation_id: annotationId,
  });
}

/**
 * Updates an existing annotation.
 */
export function updateAnnotation(annotation: AnnotationFeature): Promise<AnnotationFeature> {
  return invokeMap<AnnotationFeature>("update_annotation_cmd", { annotation });
}

/**
 * Deletes an annotation from storage by its ID.
 */
export function deleteAnnotation(annotationId: string): Promise<boolean> {
  return invokeMap<boolean>("delete_annotation_cmd", {
    annotationId,
    annotation_id: annotationId,
  });
}

/**
 * Mengambil marker foto drone dari storage (delegasi ke get_photos).
 */
export function getProjectImageMarkers(): Promise<DroneImageMetadata[]> {
  return invokeMap<DroneImageMetadata[]>("get_photos").catch(() => []);
}

export interface InstanceStatus {
  is_primary: boolean;
  is_read_only: boolean;
}

/**
 * Memeriksa apakah instansi saat ini primer atau sekunder (hanya-baca).
 */
export function getInstanceStatus(): Promise<InstanceStatus> {
  return invokeMap<InstanceStatus>("get_instance_status").catch(() => ({
    is_primary: false,
    is_read_only: true,
  }));
}

/**
 * Mengambil daftar layer dari storage (delegasi ke get_saved_layers).
 */
export function getInitialMapLayers(): Promise<LayerState[]> {
  return invokeMap<LayerState[]>("get_saved_layers").catch(() => []);
}

export function saveImageFromPath(sourcePath: string): Promise<StoredImageMetadata> {
  return invokeMap<StoredImageMetadata>("save_image_from_path", { sourcePath });
}

export function getSavedImages(): Promise<StoredImageMetadata[]> {
  return invokeMap<StoredImageMetadata[]>("get_saved_images");
}

export function deleteSavedImage(category: string, filename: string): Promise<boolean> {
  return invokeMap<boolean>("delete_saved_image", { category, filename });
}

export function importOrthoImage(params: ImportImageParams): Promise<StoredImageMetadata> {
  return invokeMap<StoredImageMetadata>("import_ortho_image", {
    payload: {
      file_path: params.filePath,
      west: params.west,
      south: params.south,
      east: params.east,
      north: params.north,
    },
  });
}

/**
 * Mengambil daftar seluruh citra orthomosaic yang tersimpan di storage (termasuk yang rusak).
 */
export function listOrthoImages(): Promise<StoredImageMetadata[]> {
  return invokeMap<StoredImageMetadata[]>("list_ortho_images");
}

/**
 * Mengambil metadata satu citra orthomosaic berdasarkan ID.
 */
export function getOrthoImage(id: string): Promise<StoredImageMetadata> {
  return invokeMap<StoredImageMetadata>("get_ortho_image", { id });
}

/**
 * Memperbarui nama dan/atau koordinat bounds citra orthomosaic.
 */
export function updateOrthoImage(params: UpdateOrthoImageParams): Promise<StoredImageMetadata> {
  return invokeMap<StoredImageMetadata>("update_ortho_image", {
    payload: {
      id: params.id,
      filename: params.filename,
      west: params.west,
      south: params.south,
      east: params.east,
      north: params.north,
    },
  });
}

/**
 * Mengambil ringkasan data cascade (layer dan anotasi terikat) sebelum konfirmasi hapus.
 */
export function getOrthoCascadeSummary(id: string): Promise<DeleteOrthoSummary> {
  return invokeMap<DeleteOrthoSummary>("get_ortho_cascade_summary", { id });
}

/**
 * Menghapus satu citra orthomosaic secara tuntas (2 fase dengan rollback jika gagal).
 */
export function deleteOrthoImage(id: string): Promise<DeleteOrthoSummary> {
  return invokeMap<DeleteOrthoSummary>("delete_ortho_image", { id });
}

/**
 * Memulai proses impor citra orthomosaic dual-mode (Opsi A atau Opsi B) secara asinkron.
 * Mengembalikan `job_id` unik untuk memantau status atau membatalkan proses.
 */
export function startImportOrthoJob(params: ImportDualModeParams): Promise<string> {
  return invokeMap<string>("start_import_ortho_job", {
    payload: {
      mode: params.mode,
      file_path: params.filePath,
      save_original: params.saveOriginal,
      manual_bounds: params.manualBounds,
      custom_filename: params.customFilename,
    },
  });
}

/**
 * Membatalkan proses impor citra orthomosaic yang sedang berlangsung.
 */
export function cancelImportJob(jobId: string): Promise<void> {
  return invokeMap<void>("cancel_import_job", { jobId });
}

/**
 * Mengambil status pekerjaan impor dan persentase progres saat ini dari backend.
 */
export function getImportJobStatus(jobId: string): Promise<JobRecord> {
  return invokeMap<JobRecord>("get_import_job_status", { jobId });
}

/**
 * Melakukan validasi awal header GeoTIFF dan mengekstrak proyeksi/bounding box sebelum impor dimulai.
 */
export function inspectGeotiffHeader(filePath: string): Promise<GeotiffInspectionResult> {
  return invokeMap<GeotiffInspectionResult>("inspect_geotiff_header", {
    filePath,
    file_path: filePath,
  });
}

// ── Spatial Results (Hasil Analisis) ────────────────────────────────────────

/**
 * Mengambil ringkasan seluruh hasil analitik spasial dari `/map_storage/spatial_results/`.
 */
export function getSavedSpatialResultsSummary(): Promise<SpatialResultSummary[]> {
  return invokeMap<SpatialResultSummary[]>("get_saved_spatial_results_summary");
}

/**
 * Mengambil detail lengkap satu hasil analitik spasial berdasarkan layer_id.
 */
export function getSavedSpatialResult(layerId: string): Promise<LayerPayload> {
  return invokeMap<LayerPayload>("get_saved_spatial_result", { layerId });
}

/**
 * Memperbarui field aman hasil analitik spasial (name, display_color, icon).
 */
export function updateSavedSpatialResult(
  payload: UpdateSpatialResultPayload,
): Promise<LayerPayload> {
  return invokeMap<LayerPayload>("update_saved_spatial_result", { payload });
}

/**
 * Menghitung dependensi layer terkait sebelum dialog konfirmasi hapus.
 */
export function countSpatialResultDependents(id: string): Promise<DeleteSpatialResultSummary> {
  return invokeMap<DeleteSpatialResultSummary>("count_spatial_result_dependents", { id });
}

/**
 * Memeriksa apakah layer hasil analisis memiliki edit manual (custom fields atau properti yang diedit).
 */
export function checkSpatialResultManualEdits(layerId: string): Promise<ManualEditsCheckResult> {
  return invokeMap<ManualEditsCheckResult>("check_spatial_result_manual_edits", { layerId });
}

/**
 * Mengambil canonical path absolut map_storage dari backend.
 */
export function getStorageRootPath(): Promise<string> {
  return invokeMap<string>("get_storage_root_path");
}
