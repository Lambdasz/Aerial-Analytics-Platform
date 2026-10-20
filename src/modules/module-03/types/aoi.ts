/**
 * TypeScript types mirroring the Rust geometry & AOI data structures.
 *
 * Coordinate convention: `[longitude, latitude]` (GeoJSON RFC 7946, WGS84).
 *
 * @module
 */

/** A single coordinate `[longitude, latitude]` in WGS84. */
export type Coordinate = [number, number];

/** GeoJSON-style polygon: array of linear rings (first = outer, rest = holes). */
export type Polygon = Coordinate[][];

/** AOI metadata (mirrors `AoiProperties` in Rust). */
export interface AoiProperties {
  aoi_id: string;
  name: string;
  calculated_area_sq_m: number;
  perimeter_m: number;
  created_at: string;
}

/** Geometry component of an AOI GeoJSON Feature. */
export interface AoiGeometry {
  type: "Polygon";
  coordinates: Polygon;
}

/**
 * Complete AOI GeoJSON Feature, ready for cross-module exchange.
 *
 * Mirrors `AoiFeature` from `geometry.rs`.
 */
export interface AoiFeature {
  type: "Feature";
  properties: AoiProperties;
  geometry: AoiGeometry;
}

/** Result payload for a single measurement operation. */
export interface MeasurementResult {
  /** The measurement mode that produced this result. */
  mode: "distance" | "area" | "perimeter";
  /** Numeric value (meters for distance/perimeter, m² for area). */
  value: number;
  /** ISO 8601 timestamp when the measurement was taken. */
  timestamp: string;
}
