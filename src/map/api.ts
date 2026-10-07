/**
 * Typed wrappers around Tauri IPC commands for AOI & spatial measurement.
 *
 * All calls go through {@link invokeMap} (from `error.ts`) so error logging
 * and structured error handling are centralised.
 *
 * @module
 */

import { invokeMap } from "./error";
import type { AoiFeature, Coordinate, Polygon } from "./types/aoi";
import type { AnnotationFeature, LayerState } from "./types/layer";

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
 * Creates a validated AOI GeoJSON Feature.
 *
 * The backend validates the polygon, computes area & perimeter, and
 * assembles the complete {@link AoiFeature}.
 */
export function createAoi(args: {
  aoiId: string;
  name: string;
  polygon: Polygon;
  createdAt: string;
}): Promise<AoiFeature> {
  return invokeMap<AoiFeature>("create_aoi_cmd", args);
}

export function toggleLayerVisibility(
  layer: LayerState,
  isVisible: boolean,
): Promise<LayerState> {
  return invokeMap<LayerState>("toggle_layer_visibility_cmd", {
    layer,
    is_visible: isVisible,
  });
}

export function setLayerOpacity(
  layer: LayerState,
  opacity: number,
): Promise<LayerState> {
  return invokeMap<LayerState>("set_layer_opacity_cmd", {
    layer,
    opacity,
  });
}

export function reorderLayerStack(
  layers: LayerState[],
  fromIndex: number,
  toIndex: number,
): Promise<LayerState[]> {
  return invokeMap<LayerState[]>("reorder_layer_stack_cmd", {
    layers,
    from_index: fromIndex,
    to_index: toIndex,
  });
}

export function filterLayersByCategory(
  layers: LayerState[],
  category: string,
): Promise<LayerState[]> {
  return invokeMap<LayerState[]>("filter_layers_by_category_cmd", {
    layers,
    category,
  });
}

export function createAnnotation(args: {
  annotationId: string;
  text: string;
  geometry: AnnotationFeature["geometry"];
  createdAt: string;
}): Promise<AnnotationFeature> {
  return invokeMap<AnnotationFeature>("create_annotation_cmd", {
    annotationId: args.annotationId,
    text: args.text,
    geometry: args.geometry,
    createdAt: args.createdAt,
  });
}