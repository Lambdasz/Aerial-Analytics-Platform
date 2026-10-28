import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { listen } from "@tauri-apps/api/event";
import { isTauri } from "@tauri-apps/api/core";
import { invokeMap } from "../services/error";
import type { MapLayerCommand, LayerPayload } from "../types/map";
import type { LayerState } from "../types/layer";
import {
  buildLayerGroupWithPoints,
  resolveFeatureArea,
  resolveMultiPolygonArea,
} from "../lib/leaflet/spatialLayerBuilder";
import { MIN_POLYGON_FEATURE_ZOOM, MIN_POINT_FEATURE_ZOOM } from "../config/mapConfig";

interface SpatialLayerEntry {
  group: L.LayerGroup;
  polygonGroup: L.LayerGroup;
  pointGroup: L.LayerGroup;
  pointLayers: L.Layer[];
}

/**
 * Mengubah opacity dan fillOpacity pada Leaflet layer secara proporsional.
 */
function applyOpacityToLayer(layer: L.Layer, opacity: number): void {
  if ("setStyle" in layer && typeof (layer as L.Path).setStyle === "function") {
    const path = layer as L.Path;
    const target = path as unknown as {
      _baseFillOpacity?: number;
      options?: { fillOpacity?: number };
    };

    if (target._baseFillOpacity === undefined) {
      target._baseFillOpacity = target.options?.fillOpacity ?? 0.35;
    }

    if (layer instanceof L.Polygon) {
      path.setStyle({
        opacity,
        fillOpacity: opacity * target._baseFillOpacity,
      });
    } else if (layer instanceof L.CircleMarker) {
      path.setStyle({
        opacity,
        fillOpacity: opacity * target._baseFillOpacity,
      });
    } else if (layer instanceof L.Polyline) {
      path.setStyle({
        opacity,
      });
    } else {
      path.setStyle({
        opacity,
        fillOpacity: opacity * target._baseFillOpacity,
      });
    }
  } else if (
    "setOpacity" in layer &&
    typeof (layer as L.Marker | L.ImageOverlay).setOpacity === "function"
  ) {
    (layer as L.Marker | L.ImageOverlay).setOpacity(opacity);
  } else if ("eachLayer" in layer && typeof (layer as L.LayerGroup).eachLayer === "function") {
    (layer as L.LayerGroup).eachLayer((child) => applyOpacityToLayer(child, opacity));
  }
}

/**
 * Memuat data hasil spasial dari backend Rust (dari `/storage/spatial_results/` atau fallback ke dummy)
 * dan merender tiap layer ke peta Leaflet.
 *
 * Mendukung filter Level of Detail (LOD) berbasis geometri:
 * - Fitur poligon, square, dan area tutupan lahan muncul pada zoom >= MIN_POLYGON_FEATURE_ZOOM (level 15).
 * - Fitur titik spasial (Point/dot/marker pohon) muncul pada zoom >= MIN_POINT_FEATURE_ZOOM (level 17).
 *
 * Layer yang didukung sesuai scope proyek:
 * - Tree detection results   (Modul 7)
 * - Vegetation detection results (Modul 4)
 * - Land-cover classification results (Modul 8)
 *
 * Layer dengan `layer_id` yang sama akan digantikan (replace)
 * jika command dipanggil ulang.
 */
export function useSpatialResultLayers(
  minPolygonZoom = MIN_POLYGON_FEATURE_ZOOM,
  minPointZoom = MIN_POINT_FEATURE_ZOOM,
  layers: LayerState[] = [],
) {
  const map = useMap();
  const layersRef = useRef<Map<string, SpatialLayerEntry>>(new Map());
  const [activeLayers, setActiveLayers] = React.useState<LayerPayload[]>([]);
  const layersPropRef = useRef<LayerState[]>(layers);

  useEffect(() => {
    layersPropRef.current = layers;
  }, [layers]);

  // Menjaga hierarki z-order geometris:
  // 1. Poligon terluas di bawah, poligon terkecil di paling atas layar
  // 2. Garis (LineString) di atas semua poligon
  const enforceZOrderHierarchy = React.useCallback(() => {
    const polygonItems: { layer: L.Polygon; area: number }[] = [];
    const lineItems: L.Path[] = [];

    layersRef.current.forEach(({ polygonGroup }) => {
      if (!map.hasLayer(polygonGroup)) return;
      polygonGroup.eachLayer((subLayer) => {
        if (subLayer instanceof L.Polygon) {
          const area = (subLayer as unknown as { _calculatedArea?: number })._calculatedArea ?? 0;
          polygonItems.push({ layer: subLayer, area });
        } else if (subLayer instanceof L.Polyline) {
          lineItems.push(subLayer);
        }
      });
    });

    // Urutkan dari area TERBESAR ke area TERKECIL
    polygonItems.sort((a, b) => b.area - a.area);

    // Bawa ke depan dari area terbesar ke terkecil:
    // Poligon terbesar di-bringToFront pertama (di bawah poligon lain)
    // Poligon terkecil di-bringToFront terakhir (berada di PALING ATAS di layar)
    // Garis (LineString) selalu di-bringToFront di atas semua poligon
    const applyBringToFront = () => {
      polygonItems.forEach(({ layer }) => {
        try {
          layer.bringToFront();
        } catch {
          // Abaikan jika layer belum siap di DOM
        }
      });
      lineItems.forEach((line) => {
        try {
          line.bringToFront();
        } catch {
          // Abaikan jika layer belum siap di DOM
        }
      });
    };

    applyBringToFront();
    requestAnimationFrame(applyBringToFront);
  }, [map]);

  // Sinkronisasi reaktif terhadap perubahan prop `layers` (opacity & on/off)
  useEffect(() => {
    // 1. Terapkan opacity ke seluruh layer yang ada
    layers.forEach((layer) => {
      const entry = layersRef.current.get(layer.layer_id);
      if (entry) {
        applyOpacityToLayer(entry.polygonGroup, layer.opacity);
        applyOpacityToLayer(entry.pointGroup, layer.opacity);
      }
    });

    // 2. Sinkronkan visibilitas dengan canvas peta
    const currentZoom = map.getZoom();
    const showPolygons = currentZoom >= minPolygonZoom;
    const showPoints = currentZoom >= minPointZoom;

    layersRef.current.forEach(({ polygonGroup, pointGroup }, layerId) => {
      const matchingLayer = layers.find((l) => l.layer_id === layerId || l.source_id === layerId);
      const isVisible = matchingLayer !== undefined ? matchingLayer.is_visible : true;

      if (!isVisible) {
        if (map.hasLayer(polygonGroup)) {
          map.removeLayer(polygonGroup);
        }
        if (map.hasLayer(pointGroup)) {
          map.removeLayer(pointGroup);
        }
        return;
      }

      const isPolygonAttached = map.hasLayer(polygonGroup);
      if (showPolygons && !isPolygonAttached) {
        map.addLayer(polygonGroup);
      } else if (!showPolygons && isPolygonAttached) {
        map.removeLayer(polygonGroup);
      }

      const isPointAttached = map.hasLayer(pointGroup);
      if (showPoints && !isPointAttached) {
        map.addLayer(pointGroup);
      } else if (!showPoints && isPointAttached) {
        map.removeLayer(pointGroup);
      }
    });

    // Pastikan hierarki z-order: poligon terkecil dan garis berada di paling atas
    enforceZOrderHierarchy();
  }, [enforceZOrderHierarchy, layers, map, minPolygonZoom, minPointZoom]);

  useEffect(() => {
    let cancelled = false;

    // Sinkronisasi visibilitas seluruh layer hasil spasial saat zoom in/out:
    // - Poligon / square muncul di zoom 15 ke atas
    // - Titik / dot / penanda kecil muncul di zoom 17 ke atas
    const syncVisibility = () => {
      const currentZoom = map.getZoom();
      const showPolygons = currentZoom >= minPolygonZoom;
      const showPoints = currentZoom >= minPointZoom;

      layersRef.current.forEach(({ polygonGroup, pointGroup }, layerId) => {
        const matchingLayer = layersPropRef.current.find(
          (l) => l.layer_id === layerId || l.source_id === layerId,
        );
        const isVisible = matchingLayer !== undefined ? matchingLayer.is_visible : true;

        if (!isVisible) {
          if (map.hasLayer(polygonGroup)) {
            map.removeLayer(polygonGroup);
          }
          if (map.hasLayer(pointGroup)) {
            map.removeLayer(pointGroup);
          }
          return;
        }

        const isPolygonAttached = map.hasLayer(polygonGroup);
        if (showPolygons && !isPolygonAttached) {
          map.addLayer(polygonGroup);
        } else if (!showPolygons && isPolygonAttached) {
          map.removeLayer(polygonGroup);
        }

        const isPointAttached = map.hasLayer(pointGroup);
        if (showPoints && !isPointAttached) {
          map.addLayer(pointGroup);
        } else if (!showPoints && isPointAttached) {
          map.removeLayer(pointGroup);
        }
      });
      enforceZOrderHierarchy();
    };

    map.on("zoom", syncVisibility);
    map.on("zoomend", syncVisibility);

    const renderCommands = (commands: MapLayerCommand[]) => {
      const loadedLayers: LayerPayload[] = [];

      // Urutkan layer agar poligon di bawah (1), garis di tengah (2), dan titik pohon paling atas (3).
      // Untuk sesama layer poligon, layer dengan area lebih luas ditaruh lebih bawah agar area dalam (lebih kecil) berada di atas.
      const sortedCommands = [...commands].sort((a, b) => {
        const getScore = (cmd: MapLayerCommand) => {
          if (cmd.payload.display_preference.type === "image_overlay") return 0;
          const hasPoint = cmd.payload.spatial_results.some((r) => r.geometry.type === "Point");
          if (hasPoint) return 3;
          const hasLine = cmd.payload.spatial_results.some(
            (r) => r.geometry.type === "LineString" || r.geometry.type === "MultiLineString",
          );
          if (hasLine) return 2;
          return 1; // Polygon / MultiPolygon
        };

        const scoreDiff = getScore(a) - getScore(b);
        if (scoreDiff !== 0) return scoreDiff;

        if (getScore(a) === 1) {
          const getTotalArea = (cmd: MapLayerCommand) =>
            cmd.payload.spatial_results.reduce((acc, r) => {
              if (r.geometry.type === "Polygon") {
                return acc + resolveFeatureArea(r.properties, r.geometry.coordinates);
              }
              if (r.geometry.type === "MultiPolygon") {
                return acc + resolveMultiPolygonArea(r.properties, r.geometry.coordinates);
              }
              return acc;
            }, 0);
          return getTotalArea(b) - getTotalArea(a);
        }

        return 0;
      });

      const currentZoom = map.getZoom();
      const showPolygons = currentZoom >= minPolygonZoom;
      const showPoints = currentZoom >= minPointZoom;

      sortedCommands.forEach(({ action, payload }) => {
        if (action !== "load_spatial_result") return;

        const existing = layersRef.current.get(payload.layer_id);
        if (existing) {
          if (map.hasLayer(existing.polygonGroup)) {
            map.removeLayer(existing.polygonGroup);
          }
          if (map.hasLayer(existing.pointGroup)) {
            map.removeLayer(existing.pointGroup);
          }
          if (map.hasLayer(existing.group)) {
            map.removeLayer(existing.group);
          }
        }

        const { group, polygonGroup, pointGroup, pointLayers } = buildLayerGroupWithPoints(
          payload,
          true,
        );

        const matching = layersPropRef.current.find(
          (l) => l.layer_id === payload.layer_id || l.source_id === payload.layer_id,
        );
        const isVisible = matching !== undefined ? matching.is_visible : true;
        const opacity = matching !== undefined ? matching.opacity : 1.0;

        if (opacity !== 1.0) {
          applyOpacityToLayer(polygonGroup, opacity);
          applyOpacityToLayer(pointGroup, opacity);
        }

        if (isVisible) {
          if (showPolygons) {
            map.addLayer(polygonGroup);
          }
          if (showPoints) {
            map.addLayer(pointGroup);
          }
        }
        layersRef.current.set(payload.layer_id, { group, polygonGroup, pointGroup, pointLayers });
        loadedLayers.push(payload);
      });
      setActiveLayers(loadedLayers);
      enforceZOrderHierarchy();
    };

    // Muat data hasil spasial dari storage `/storage/spatial_results/`
    invokeMap<LayerPayload[]>("get_saved_spatial_results")
      .then((savedPayloads) => {
        if (cancelled) return;
        if (savedPayloads && savedPayloads.length > 0) {
          const cmds: MapLayerCommand[] = savedPayloads.map((payload) => ({
            action: "load_spatial_result",
            payload,
          }));
          renderCommands(cmds);
        }
      })
      .catch((err) => {
        console.warn("Gagal memuat saved spatial layers:", err);
      });

    // Event listener jika ada spatial result baru yang disimpan
    const handleSpatialResultSaved = (event: Event) => {
      const customEvent = event as CustomEvent<LayerPayload>;
      if (customEvent.detail) {
        const payload = customEvent.detail;
        const existing = layersRef.current.get(payload.layer_id);
        if (existing) {
          if (map.hasLayer(existing.polygonGroup)) {
            map.removeLayer(existing.polygonGroup);
          }
          if (map.hasLayer(existing.pointGroup)) {
            map.removeLayer(existing.pointGroup);
          }
          if (map.hasLayer(existing.group)) {
            map.removeLayer(existing.group);
          }
        }

        const currentZoom = map.getZoom();
        const showPolygons = currentZoom >= minPolygonZoom;
        const showPoints = currentZoom >= minPointZoom;
        const { group, polygonGroup, pointGroup, pointLayers } = buildLayerGroupWithPoints(
          payload,
          true,
        );

        const matching = layersPropRef.current.find(
          (l) => l.layer_id === payload.layer_id || l.source_id === payload.layer_id,
        );
        const isVisible = matching !== undefined ? matching.is_visible : true;
        const opacity = matching !== undefined ? matching.opacity : 1.0;

        if (opacity !== 1.0) {
          applyOpacityToLayer(polygonGroup, opacity);
          applyOpacityToLayer(pointGroup, opacity);
        }

        if (isVisible) {
          if (showPolygons) {
            map.addLayer(polygonGroup);
          }
          if (showPoints) {
            map.addLayer(pointGroup);
          }
        }
        layersRef.current.set(payload.layer_id, { group, polygonGroup, pointGroup, pointLayers });
        setActiveLayers((prev) => [
          ...prev.filter((l) => l.layer_id !== payload.layer_id),
          payload,
        ]);
        enforceZOrderHierarchy();
      }
    };

    const handleSpatialResultDeleted = (event: Event) => {
      const customEvent = event as CustomEvent<{ id: string }>;
      const id = customEvent.detail?.id;
      if (!id) return;

      const existing = layersRef.current.get(id);
      if (existing) {
        if (map.hasLayer(existing.polygonGroup)) {
          map.removeLayer(existing.polygonGroup);
        }
        if (map.hasLayer(existing.pointGroup)) {
          map.removeLayer(existing.pointGroup);
        }
        if (map.hasLayer(existing.group)) {
          map.removeLayer(existing.group);
        }
        layersRef.current.delete(id);
      }
      setActiveLayers((prev) => prev.filter((l) => l.layer_id !== id));
    };

    window.addEventListener("spatial_result:saved", handleSpatialResultSaved);
    window.addEventListener("spatial_result:deleted", handleSpatialResultDeleted);

    let unlistenSaved: (() => void) | undefined;
    let unlistenDeleted: (() => void) | undefined;

    if (isTauri()) {
      void listen<LayerPayload>("spatial_result:saved", (event) => {
        handleSpatialResultSaved(
          new CustomEvent("spatial_result:saved", { detail: event.payload }),
        );
      }).then((fn) => {
        unlistenSaved = fn;
      });

      void listen<{ id: string }>("spatial_result:deleted", (event) => {
        handleSpatialResultDeleted(
          new CustomEvent("spatial_result:deleted", { detail: event.payload }),
        );
      }).then((fn) => {
        unlistenDeleted = fn;
      });
    }

    const currentLayers = layersRef.current;

    return () => {
      cancelled = true;
      window.removeEventListener("spatial_result:saved", handleSpatialResultSaved);
      window.removeEventListener("spatial_result:deleted", handleSpatialResultDeleted);
      unlistenSaved?.();
      unlistenDeleted?.();
      map.off("zoom", syncVisibility);
      map.off("zoomend", syncVisibility);
      currentLayers.forEach(({ polygonGroup, pointGroup, group }) => {
        if (map.hasLayer(polygonGroup)) {
          map.removeLayer(polygonGroup);
        }
        if (map.hasLayer(pointGroup)) {
          map.removeLayer(pointGroup);
        }
        if (map.hasLayer(group)) {
          map.removeLayer(group);
        }
      });
      currentLayers.clear();
      setActiveLayers([]);
    };
  }, [enforceZOrderHierarchy, map, minPolygonZoom, minPointZoom]);

  return { activeLayers };
}
