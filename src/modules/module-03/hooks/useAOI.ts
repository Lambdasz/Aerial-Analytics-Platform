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

import { useCallback, useEffect, useRef, useState } from "react";
import type { Map as LeafletMap } from "leaflet";
import * as L from "leaflet";
import "../leaflet-setup";
import "@geoman-io/leaflet-geoman-free";

import { saveNewAoi } from "../services/api";
import type { AoiFeature, Polygon } from "../types/aoi";
import { latLngsToPolygon } from "../lib/geoJson";

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
  /** Load an existing AOI polygon into Geoman editing mode on the map. */
  loadAoiForEditing: (targetAoi: AoiFeature, map: LeafletMap) => void;
  /** Validate, compute metrics, and persist the draft polygon as an AOI. */
  saveAoi: (aoiId: string, name: string, aoiType?: string) => Promise<AoiFeature>;
}

/**
 * Safely detach and disable Geoman on a Leaflet layer, then remove it from the map.
 */
function detachDrawnLayer(layer: L.Layer | null, map: LeafletMap | null) {
  if (!layer) return;

  // Unbind Geoman events from the layer
  layer.off("pm:edit");
  layer.off("pm:update");
  layer.off("pm:vertexchange");
  layer.off("pm:markerdragend");
  layer.off("pm:vertexadded");
  layer.off("pm:vertexremoved");

  const pmLayer = layer as unknown as { pm?: { disable?: () => void } };
  if (pmLayer.pm && typeof pmLayer.pm.disable === "function") {
    pmLayer.pm.disable();
  }

  if (map && map.hasLayer(layer)) {
    map.removeLayer(layer);
  }
}

/**
 * Converts a Leaflet layer's LatLngs to a GeoJSON Polygon (RFC 7946).
 *
 * Leaflet uses `[lat, lng]`; GeoJSON uses `[lng, lat]`.
 */
function layerToPolygon(layer: L.Layer): Polygon {
  if (!layer || typeof (layer as L.Polygon).getLatLngs !== "function") {
    return [];
  }
  const latLngs = (layer as L.Polygon).getLatLngs();
  return latLngsToPolygon(latLngs);
}

/**
 * React hook for AOI drawing, editing, and persistence.
 */
export function useAOI(): UseAOIReturn {
  const [aoi, setAoi] = useState<AoiFeature | null>(null);
  const [draftPolygon, setDraftPolygon] = useState<Polygon | null>(null);
  const [isDrawing, setIsDrawing] = useState(false);
  const [isSaving, setIsSaving] = useState(false);

  // Keep a ref to the drawn layer so we can remove it on clear/cancel.
  const drawnLayerRef = useRef<L.Layer | null>(null);
  const mapRef = useRef<LeafletMap | null>(null);

  // ── Cleanup on unmount ──────────────────────────────────────────────
  // Memastikan tidak ada layer oranye / handle Geoman yang tertinggal di peta saat pindah section
  useEffect(() => {
    return () => {
      if (drawnLayerRef.current) {
        detachDrawnLayer(drawnLayerRef.current, mapRef.current);
        drawnLayerRef.current = null;
      }
      if (mapRef.current) {
        mapRef.current.pm?.disableDraw?.();
      }
      window.dispatchEvent(new CustomEvent("aoi:editing-stopped"));
    };
  }, []);

  // ── Geoman event handlers ──────────────────────────────────────────

  const handleCreate = useCallback((e: { layer: L.Layer }) => {
    drawnLayerRef.current = e.layer;
    setDraftPolygon(layerToPolygon(e.layer));
    setIsDrawing(false);

    // Sinkronisasi koordinat jika poligon yang baru digambar diedit sebelum disimpan
    const syncCreated = () => {
      if (drawnLayerRef.current) {
        setDraftPolygon(layerToPolygon(drawnLayerRef.current));
      }
    };
    e.layer.on("pm:edit", syncCreated);
    e.layer.on("pm:update", syncCreated);
    e.layer.on("pm:markerdragend", syncCreated);
  }, []);

  // ── Public API ─────────────────────────────────────────────────────

  const startDrawing = useCallback(
    (map: LeafletMap) => {
      mapRef.current = map;

      // Bersihkan layer draft sebelumnya jika belum tersimpan
      if (drawnLayerRef.current) {
        detachDrawnLayer(drawnLayerRef.current, map);
        drawnLayerRef.current = null;
      }
      setDraftPolygon(null);
      window.dispatchEvent(new CustomEvent("aoi:editing-stopped"));

      // Pastikan listener Geoman terpasang
      map.off("pm:create", handleCreate as L.LeafletEventHandlerFn);
      map.on("pm:create", handleCreate as L.LeafletEventHandlerFn);

      // Aktifkan mode drawing polygon Geoman
      map.pm.enableDraw("Polygon");
      setIsDrawing(true);
    },
    [handleCreate],
  );

  const stopDrawing = useCallback((map: LeafletMap) => {
    map.pm.disableDraw();
    setIsDrawing(false);

    // Hapus draft layer dari map jika ada
    if (drawnLayerRef.current) {
      detachDrawnLayer(drawnLayerRef.current, map);
      drawnLayerRef.current = null;
    }
    setDraftPolygon(null);
    window.dispatchEvent(new CustomEvent("aoi:editing-stopped"));
  }, []);

  const clearAoi = useCallback(() => {
    if (drawnLayerRef.current) {
      detachDrawnLayer(drawnLayerRef.current, mapRef.current);
      drawnLayerRef.current = null;
    }
    setDraftPolygon(null);
    setAoi(null);
    window.dispatchEvent(new CustomEvent("aoi:editing-stopped"));
  }, []);

  const saveAoi = useCallback(
    async (aoiId: string, name: string, aoiType?: string): Promise<AoiFeature> => {
      // Prioritaskan koordinat riil langsung dari layer polygon aktif di peta
      const currentPolygon =
        drawnLayerRef.current &&
        typeof (drawnLayerRef.current as L.Polygon).getLatLngs === "function"
          ? layerToPolygon(drawnLayerRef.current)
          : draftPolygon;

      if (!currentPolygon || currentPolygon.length === 0 || currentPolygon[0].length < 4) {
        throw new Error(
          "Poligon tidak valid atau belum selesai digambar (minimal membutuhkan 3 titik sudut).",
        );
      }

      setIsSaving(true);
      try {
        const feature = await saveNewAoi({
          aoiId,
          name,
          aoiType,
          polygon: currentPolygon,
          createdAt: new Date().toISOString(),
        });

        // Hapus layer draft sementara Geoman agar digantikan oleh SavedAoiLayer permanen
        if (drawnLayerRef.current) {
          detachDrawnLayer(drawnLayerRef.current, mapRef.current);
          drawnLayerRef.current = null;
        }

        // Beritahu layer peta bahwa mode edit telah selesai dan data berhasil diperbarui
        window.dispatchEvent(new CustomEvent("aoi:editing-stopped", { detail: { aoiId } }));
        window.dispatchEvent(new CustomEvent("aoi:saved", { detail: feature }));

        setAoi(feature);
        setDraftPolygon(null);
        return feature;
      } finally {
        setIsSaving(false);
      }
    },
    [draftPolygon],
  );

  const loadAoiForEditing = useCallback((targetAoi: AoiFeature, map: LeafletMap) => {
    mapRef.current = map;

    // Bersihkan layer draft sebelumnya jika ada
    if (drawnLayerRef.current) {
      detachDrawnLayer(drawnLayerRef.current, map);
      drawnLayerRef.current = null;
    }

    // Beritahu layer tersimpan bahwa AOI ini sedang diedit (SavedAoiLayer menyembunyikan versi biru sementara)
    window.dispatchEvent(
      new CustomEvent("aoi:editing-started", {
        detail: { aoiId: targetAoi.properties.aoi_id },
      }),
    );

    // GeoJSON [lng, lat] -> Leaflet [lat, lng]
    const rings = targetAoi.geometry.coordinates.map((ring) =>
      ring.map(([lng, lat]) => [lat, lng] as [number, number]),
    );

    const polygon = L.polygon(rings, {
      color: "#ff9500",
      weight: 3,
      dashArray: "6, 6",
      fillColor: "#ff9500",
      fillOpacity: 0.25,
    }).addTo(map);

    drawnLayerRef.current = polygon;
    setDraftPolygon(targetAoi.geometry.coordinates);
    setAoi(targetAoi);
    setIsDrawing(false);

    // Sinkronisasi koordinat saat titik sudut digeser/ditambah/dihapus
    const syncPolygonFromLayer = () => {
      if (drawnLayerRef.current) {
        const poly = layerToPolygon(drawnLayerRef.current);
        if (poly.length > 0 && poly[0].length >= 4) {
          setDraftPolygon(poly);
        }
      }
    };

    polygon.on("pm:edit", syncPolygonFromLayer);
    polygon.on("pm:update", syncPolygonFromLayer);
    polygon.on("pm:vertexchange", syncPolygonFromLayer);
    polygon.on("pm:markerdragend", syncPolygonFromLayer);
    polygon.on("pm:vertexadded", syncPolygonFromLayer);
    polygon.on("pm:vertexremoved", syncPolygonFromLayer);

    if ((polygon as unknown as { pm?: { enable: (opts?: unknown) => void } }).pm) {
      (polygon as unknown as { pm: { enable: (opts?: unknown) => void } }).pm.enable({
        allowSelfIntersection: false,
      });
    }

    map.fitBounds(polygon.getBounds(), { padding: [50, 50], animate: true });
  }, []);

  return {
    aoi,
    draftPolygon,
    isDrawing,
    isSaving,
    startDrawing,
    stopDrawing,
    clearAoi,
    loadAoiForEditing,
    saveAoi,
  };
}
