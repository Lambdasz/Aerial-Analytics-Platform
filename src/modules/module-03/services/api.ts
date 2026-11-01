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
export async function measureDistance(pointA: Coordinate, pointB: Coordinate): Promise<number> {
  const res = await invokeMap<number | { distanceMeters: number }>("measure_distance", {
    request: { pointA, pointB },
    pointA,
    pointB,
  });
  if (typeof res === "number") return res;
  return res?.distanceMeters ?? 0;
}

/**
 * Calculates the area of a polygon (spherical excess, WGS84).
 *
 * @param polygon — GeoJSON-style polygon rings.
 * @returns Area in **square meters**.
 */
export async function measureArea(polygon: Polygon): Promise<number> {
  const res = await invokeMap<number | { areaSquareMeters: number }>("measure_area", {
    request: { polygon },
    polygon,
  });
  if (typeof res === "number") return res;
  return res?.areaSquareMeters ?? 0;
}

/**
 * Calculates the perimeter of a polygon.
 *
 * @param polygon — GeoJSON-style polygon rings.
 * @returns Perimeter in **meters**.
 */
export async function measurePerimeter(polygon: Polygon): Promise<number> {
  const res = await invokeMap<number | { perimeterMeters: number }>("measure_perimeter", {
    request: { polygon },
    polygon,
  });
  if (typeof res === "number") return res;
  return res?.perimeterMeters ?? 0;
}

/**
 * Creates and persists an AOI GeoJSON Feature atomically in a single IPC call.
 */
export async function saveNewAoi(args: {
  aoiId: string;
  name: string;
  aoiType?: string;
  polygon: Polygon;
  createdAt?: string;
}): Promise<AoiFeature> {
  const res = await invokeMap<AoiFeature | { aoi: AoiFeature }>("save_new_aoi_cmd", {
    request: args,
    ...args,
  });
  return res && "aoi" in res ? res.aoi : res;
}

/**
 * Creates a validated AOI GeoJSON Feature.
 *
 * The backend validates the polygon, computes area & perimeter, and
 * assembles the complete {@link AoiFeature}.
 */
export async function createAoi(args: {
  aoiId: string;
  name: string;
  aoiType?: string;
  polygon: Polygon;
  createdAt: string;
}): Promise<AoiFeature> {
  const res = await invokeMap<AoiFeature | { aoi: AoiFeature }>("create_aoi_cmd", {
    request: args,
    ...args,
  });
  return res && "aoi" in res ? res.aoi : res;
}

/**
 * Persists an AOI GeoJSON Feature to the backend `/storage/aois/{aoi_id}.json`.
 */
export async function saveAoi(aoi: AoiFeature): Promise<AoiFeature> {
  const res = await invokeMap<AoiFeature | { aoi: AoiFeature }>("save_aoi", {
    request: { aoi },
    aoi,
  });
  return res && "aoi" in res ? res.aoi : res;
}

/**
 * Retrieves all saved AOIs from `/storage/aois/`.
 */
export async function getSavedAois(): Promise<AoiFeature[]> {
  const res = await invokeMap<AoiFeature[] | { aois: AoiFeature[]; total: number }>(
    "get_saved_aois",
  );
  if (Array.isArray(res)) return res;
  return res?.aois ?? [];
}

/**
 * Deletes a saved AOI from storage by its ID.
 */
export async function deleteSavedAoi(aoiId: string): Promise<boolean> {
  const res = await invokeMap<boolean | { success: boolean; aoiId: string }>("delete_saved_aoi", {
    request: { aoiId },
    aoiId,
    aoi_id: aoiId,
  });
  if (typeof res === "boolean") return res;
  return res?.success ?? false;
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
export async function getSavedSpatialResults(): Promise<LayerPayload[]> {
  const res = await invokeMap<LayerPayload[] | { results: LayerPayload[]; total: number }>(
    "get_saved_spatial_results",
  );
  if (Array.isArray(res)) return res;
  return res?.results ?? [];
}

/**
 * Deletes a saved spatial result from storage by its layer ID.
 */
export function deleteSavedSpatialResult(layerId: string): Promise<boolean> {
  return invokeMap<boolean>("delete_saved_spatial_result", { layerId });
}

export async function createAnnotation(args: {
  annotationId?: string;
  id?: string;
  text: string;
  geometry: AnnotationFeature["geometry"];
  createdAt: string;
}): Promise<AnnotationFeature> {
  const finalId = args.id || args.annotationId || "";
  const payload = {
    annotationId: finalId,
    annotation_id: finalId,
    text: args.text,
    geometry: args.geometry,
    createdAt: args.createdAt,
    created_at: args.createdAt,
  };
  const res = await invokeMap<AnnotationFeature | { annotation: AnnotationFeature }>(
    "create_annotation_cmd",
    {
      request: {
        annotationId: finalId,
        text: args.text,
        geometry: args.geometry,
        createdAt: args.createdAt,
      },
      ...payload,
    },
  );
  return res && "annotation" in res ? res.annotation : res;
}

/**
 * Retrieves all saved annotations from `/map_storage/annotations/`.
 */
export async function getSavedAnnotations(): Promise<AnnotationFeature[]> {
  const res = await invokeMap<
    AnnotationFeature[] | { annotations: AnnotationFeature[]; total: number }
  >("get_saved_annotations");
  if (Array.isArray(res)) return res;
  return (res as { annotations?: AnnotationFeature[] })?.annotations ?? [];
}

/**
 * Retrieves a single annotation by ID from `/map_storage/annotations/`.
 */
export function getAnnotationById(annotationId: string): Promise<AnnotationFeature> {
  return invokeMap<AnnotationFeature>("get_annotation_by_id", {
    request: { annotationId },
    annotationId,
    annotation_id: annotationId,
  });
}

/**
 * Updates an existing annotation.
 */
export function updateAnnotation(annotation: AnnotationFeature): Promise<AnnotationFeature> {
  return invokeMap<AnnotationFeature>("update_annotation_cmd", {
    request: { annotation },
    annotation,
  });
}

/**
 * Deletes an annotation from storage by its ID.
 */
export async function deleteAnnotation(annotationId: string): Promise<boolean> {
  const res = await invokeMap<boolean | { success: boolean; annotationId: string }>(
    "delete_annotation_cmd",
    {
      request: { annotationId },
      annotationId,
      annotation_id: annotationId,
    },
  );
  if (typeof res === "boolean") return res;
  return res?.success ?? false;
}

/**
 * Mengambil marker foto drone dari storage (delegasi ke get_photos).
 */
export async function getProjectImageMarkers(): Promise<DroneImageMetadata[]> {
  const res = await invokeMap<
    DroneImageMetadata[] | { photos: DroneImageMetadata[]; total: number }
  >("get_photos").catch(() => []);
  if (Array.isArray(res)) return res;
  return (res as { photos?: DroneImageMetadata[] })?.photos ?? [];
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
export async function getInitialMapLayers(): Promise<LayerState[]> {
  const res = await invokeMap<LayerState[] | { layers: LayerState[]; total: number }>(
    "get_saved_layers",
  ).catch(() => []);
  if (Array.isArray(res)) return res;
  return (res as { layers?: LayerState[] })?.layers ?? [];
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
export async function listOrthoImages(): Promise<StoredImageMetadata[]> {
  const res = await invokeMap<
    StoredImageMetadata[] | { images: StoredImageMetadata[]; total: number }
  >("list_ortho_images");
  if (Array.isArray(res)) return res;
  return res?.images ?? [];
}

/**
 * Mengambil metadata satu citra orthomosaic berdasarkan ID.
 */
export async function getOrthoImage(id: string): Promise<StoredImageMetadata> {
  const res = await invokeMap<StoredImageMetadata | { image: StoredImageMetadata }>(
    "get_ortho_image",
    {
      request: { id },
      id,
    },
  );
  return res && "image" in res ? res.image : res;
}

/**
 * Memperbarui nama dan/atau koordinat bounds citra orthomosaic.
 */
export async function updateOrthoImage(
  params: UpdateOrthoImageParams,
): Promise<StoredImageMetadata> {
  const payload = {
    id: params.id,
    filename: params.filename,
    west: params.west,
    south: params.south,
    east: params.east,
    north: params.north,
  };
  const res = await invokeMap<StoredImageMetadata | { image: StoredImageMetadata }>(
    "update_ortho_image",
    {
      request: { payload },
      payload,
    },
  );
  return res && "image" in res ? res.image : res;
}

/**
 * Mengambil ringkasan data cascade (layer dan anotasi terikat) sebelum konfirmasi hapus.
 */
export async function getOrthoCascadeSummary(id: string): Promise<DeleteOrthoSummary> {
  const res = await invokeMap<DeleteOrthoSummary | { summary: DeleteOrthoSummary }>(
    "get_ortho_cascade_summary",
    {
      request: { id },
      id,
    },
  );
  return res && "summary" in res ? res.summary : res;
}

/**
 * Menghapus satu citra orthomosaic secara tuntas (2 fase dengan rollback jika gagal).
 */
export async function deleteOrthoImage(id: string): Promise<DeleteOrthoSummary> {
  const res = await invokeMap<DeleteOrthoSummary | { summary: DeleteOrthoSummary }>(
    "delete_ortho_image",
    {
      request: { id },
      id,
    },
  );
  return res && "summary" in res ? res.summary : res;
}

/**
 * Memulai proses impor citra orthomosaic dual-mode (Opsi A atau Opsi B) secara asinkron.
 * Mengembalikan `job_id` unik untuk memantau status atau membatalkan proses.
 */
export async function startImportOrthoJob(params: ImportDualModeParams): Promise<string> {
  const payload = {
    mode: params.mode,
    file_path: params.filePath,
    save_original: params.saveOriginal,
    manual_bounds: params.manualBounds,
    custom_filename: params.customFilename,
  };
  const res = await invokeMap<string | { jobId: string }>("start_import_ortho_job", {
    request: { payload },
    payload,
  });
  if (typeof res === "string") return res;
  return res?.jobId ?? "";
}

/**
 * Membatalkan proses impor citra orthomosaic yang sedang berlangsung.
 */
export function cancelImportJob(jobId: string): Promise<void> {
  return invokeMap<void>("cancel_import_job", {
    request: { jobId },
    jobId,
    job_id: jobId,
  });
}

/**
 * Mengambil status pekerjaan impor dan persentase progres saat ini dari backend.
 */
export async function getImportJobStatus(jobId: string): Promise<JobRecord> {
  const res = await invokeMap<JobRecord | { record: JobRecord }>("get_import_job_status", {
    request: { jobId },
    jobId,
    job_id: jobId,
  });
  return res && "record" in res ? res.record : res;
}

/**
 * Melakukan validasi awal header GeoTIFF dan mengekstrak proyeksi/bounding box sebelum impor dimulai.
 */
export function inspectGeotiffHeader(filePath: string): Promise<GeotiffInspectionResult> {
  return invokeMap<GeotiffInspectionResult>("inspect_geotiff_header", {
    request: { filePath },
    filePath,
    file_path: filePath,
  });
}

// ── Spatial Results (Hasil Analisis) ────────────────────────────────────────

/**
 * Mengambil ringkasan seluruh hasil analitik spasial dari `/map_storage/spatial_results/`.
 */
export async function getSavedSpatialResultsSummary(): Promise<SpatialResultSummary[]> {
  const res = await invokeMap<
    SpatialResultSummary[] | { summaries: SpatialResultSummary[]; total: number }
  >("get_saved_spatial_results_summary");
  if (Array.isArray(res)) return res;
  return res?.summaries ?? [];
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
