/**
 * Kontrak data spasial yang diterima dari backend Rust via Tauri.
 *
 * Hanya mencakup modul analitik yang berada dalam scope proyek:
 * - Modul 4 (RGB Vegetation Detection)  → VegetationProperties
 * - Modul 7 (Tree Detection & Counting) → TreeProperties
 * - Modul 8 (Land-Cover Classification) → LandCoverProperties
 *
 * Catatan: BuildingProperties TIDAK termasuk — deteksi bangunan individual
 * bukan bagian dari scope proyek ini. Modul 8 mengklasifikasikan
 * tutupan lahan secara visual (termasuk 'built_area' sebagai kelas area,
 * bukan deteksi objek bangunan individual).
 */

export interface MapLayerCommand {
  action: string;
  payload: LayerPayload;
}

export interface LayerPayload {
  layer_id: string;
  layer_name: string;
  display_preference: LayerDisplayMode;
  spatial_results: SpatialResult[];
}

export type LayerDisplayMode =
  | { type: "polygon"; color: string; opacity: number }
  | { type: "point"; color: string; icon: string }
  | {
      type: "image_overlay";
      image_url: string;
      opacity: number;
      bounds: [[number, number], [number, number]];
    };

export interface SpatialResult {
  source: string;
  geometry: SpatialGeometry;
  properties: SpatialProperties;
}

// Koordinat mengikuti urutan GeoJSON: [longitude, latitude]
export type SpatialGeometry =
  | { type: "Point"; coordinates: [number, number] }
  | { type: "Polygon"; coordinates: number[][][] }
  | { type: "LineString"; coordinates: [number, number][] }
  | { type: "MultiLineString"; coordinates: [number, number][][] };

/**
 * Union tipe properti untuk seluruh hasil analitik yang didukung.
 * Sesuai dengan `SpatialProperties` enum di backend Rust.
 */
export type SpatialProperties =
  TreeProperties | VegetationProperties | LandCoverProperties | InspectionPathProperties;

/** Modul 7 — Tree Detection & Counting */
export interface TreeProperties {
  tree_id: string;
  confidence: number;
  height_est_m: number;
}

/** Modul 4 — RGB Vegetation Detection */
export interface VegetationProperties {
  /** Jenis indeks yang digunakan: "ExG", "ExR", "VARI", dll. */
  vegetation_index_type: string;
  mean_greenness_score: number;
  area_sqm: number;
}

/**
 * Modul 8 — RGB Land-Cover Classification
 *
 * Kelas yang mungkin muncul: "vegetation", "bare_soil", "water", "built_area"
 * 'built_area' adalah kelas tutupan lahan (bukan deteksi bangunan individual).
 */
export interface LandCoverProperties {
  land_cover_class: string;
  confidence: number;
  area_sqm: number;
}

/** Jalur Inspeksi */
export interface InspectionPathProperties {
  path_id: string;
  type: string;
  length_m: number;
}

// Type guards untuk membedakan tipe properties
export function isTreeProperties(p: SpatialProperties): p is TreeProperties {
  return "tree_id" in p;
}

export function isVegetationProperties(p: SpatialProperties): p is VegetationProperties {
  return "vegetation_index_type" in p;
}

export function isLandCoverProperties(p: SpatialProperties): p is LandCoverProperties {
  return "land_cover_class" in p;
}

export function isInspectionPathProperties(p: SpatialProperties): p is InspectionPathProperties {
  return "path_id" in p;
}
