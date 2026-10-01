/**
 * Hook for managing Area of Interest (AOI) lifecycle.
 *
 * Bridges imperative Geoman events (`pm:create`, `pm:edit`, `pm:remove`) to
 * React state, keeping the map component clean. The component only needs to
 * call {@link useAOI} and consume the returned API — no direct `setState`
 * or event wiring in the render tree.
 *
 * ## Coordinate convention
 *
 * All coordinates are `[longitude, latitude]` (GeoJSON RFC 7946).
 * Leaflet's native `LatLng` (`[lat, lng]`) is converted to `[lng, lat]`
 * before being sent to the backend.
 *
 * @module
 */

import { useCallback, useRef, useState } from "react";
import type { Map as LeafletMap } from "leaflet";
import type L from "leaflet";
import "@geoman-io/leaflet-geoman-free";

import { createAoi } from "../api";
import type { AoiFeature, Polygon } from "../types/aoi";

/** Public API surface returned by {@link useAOI}. */
export interface UseAOIReturn {
  /** The current AOI feature (null until saved). */
  aoi: AoiFeature | null;
  /** The raw polygon being drawn/edited (before backend validation). */
  draftPolygon: Polygon | null;
  /** Whether the user is currently drawing an AOI polygon. */
  isDrawing: boolean;
  /** Whether a `saveAoi` call is in flight. */
  isSaving: boolean;
  /** Start Geoman polygon drawing mode on the given map instance. */
  startDrawing: (map: LeafletMap) => void;
  /** Cancel drawing mode and discard the draft polygon. */
  stopDrawing: (map: LeafletMap) => void;
  /** Clear the current AOI and draft polygon. */
  clearAoi: () => void;
  /** Validate, compute metrics, and persist the draft polygon as an AOI. */
  saveAoi: (aoiId: string, name: string) => Promise<AoiFeature>;
}

/**
 * Converts a Leaflet layer's LatLngs to a GeoJSON Polygon (RFC 7946).
 *
 * Leaflet uses `[lat, lng]`; GeoJSON uses `[lng, lat]`.
 */
function layerToPolygon(layer: L.Layer): Polygon {
  // Leaflet's Polygon.getLatLngs() returns LatLng[][] (array of rings)
  const latLngs = (layer as L.Polygon).getLatLngs() as L.LatLng[][];

  return latLngs.map((ring) => {
    const coords = ring.map((ll): [number, number] => [ll.lng, ll.lat]);
    // Ensure the ring is closed (GeoJSON requirement)
    if (
      coords.length > 0 &&
      (coords[0][0] !== coords[coords.length - 1][0] ||
        coords[0][1] !== coords[coords.length - 1][1])
    ) {
      coords.push([...coords[0]]);
    }
    return coords;
  });
}

/**
 * React hook for AOI drawing, editing, and persistence.
 *
 * @example
 * ```tsx
 * function AoiToolbar() {
 *   const map = useMap();
 *   const { aoi, isDrawing, startDrawing, stopDrawing, saveAoi } = useAOI();
 *
 *   return (
 *     <>
 *       <button onClick={() => startDrawing(map)}>Draw AOI</button>
 *       {isDrawing && <button onClick={() => stopDrawing(map)}>Cancel</button>}
 *       {!isDrawing && draftPolygon && (
 *         <button onClick={() => saveAoi("aoi-1", "My Area")}>Save</button>
 *       )}
 *     </>
 *   );
 * }
 * ```
 */
export function useAOI(): UseAOIReturn {
  const [aoi, setAoi] = useState<AoiFeature | null>(null);
  const [draftPolygon, setDraftPolygon] = useState<Polygon | null>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Keep a ref to the drawn layer so we can remove it on clear/cancel.
  const drawnLayerRef = useRef<L.Layer | null>(null);

  // ── Geoman event handlers ──────────────────────────────────────────

  const handleCreate = useCallback((e: { layer: L.Layer }) => {
    drawnLayerRef.current = e.layer;
    setDraftPolygon(layerToPolygon(e.layer));
    setIsDrawing(false);
  }, []);

  const handleEdit = useCallback((e: { layer: L.Layer }) => {
    setDraftPolygon(layerToPolygon(e.layer));
  }, []);

  const handleRemove = useCallback(() => {
    drawnLayerRef.current = null;
    setDraftPolygon(null);
    setAoi(null);
  }, []);

  // ── Public API ─────────────────────────────────────────────────────

  const startDrawing = useCallback(
    (map: LeafletMap) => {
      // Attach Geoman listeners (idempotent — Leaflet ignores duplicate adds
      // for the same handler reference thanks to useCallback stability).
      map.on("pm:create", handleCreate as L.LeafletEventHandlerFn);
      map.on("pm:edit", handleEdit as L.LeafletEventHandlerFn);
      map.on("pm:remove", handleRemove as L.LeafletEventHandlerFn);

      // Enable Geoman polygon drawing mode
      map.pm.enableDraw("Polygon");
      setIsDrawing(true);
    },
    [handleCreate, handleEdit, handleRemove],
  );

  const stopDrawing = useCallback((map: LeafletMap) => {
    map.pm.disableDraw();
    setIsDrawing(false);

    // Remove the draft layer from the map if it exists
    if (drawnLayerRef.current) {
      map.removeLayer(drawnLayerRef.current);
      drawnLayerRef.current = null;
    }
    setDraftPolygon(null);
  }, []);

  const clearAoi = useCallback(() => {
    drawnLayerRef.current = null;
    setDraftPolygon(null);
    setAoi(null);
  }, []);

  const saveAoi = useCallback(
    async (aoiId: string, name: string): Promise<AoiFeature> => {
      if (!draftPolygon) {
        throw new Error("No draft polygon to save. Draw a polygon first.");
      }

      setIsSaving(true);
      try {
        const feature = await createAoi({
          aoiId,
          name,
          polygon: draftPolygon,
          createdAt: new Date().toISOString(),
        });
        setAoi(feature);
        return feature;
      } finally {
        setIsSaving(false);
      }
    },
    [draftPolygon],
  );

  return {
    aoi,
    draftPolygon,
    isDrawing,
    isSaving,
    startDrawing,
    stopDrawing,
    clearAoi,
    saveAoi,
  };
}
