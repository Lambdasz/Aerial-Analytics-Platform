import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { invokeMap } from "../services/error";
import type { MapLayerCommand, LayerPayload } from "../types/map";
import { buildLayerGroup } from "../lib/leaflet/spatialLayerBuilder";

/**
 * Memuat data dummy hasil spasial dari backend Rust (command
 * `get_dummy_spatial_layers`) dan merender tiap layer ke peta Leaflet.
 *
 * Layer yang didukung sesuai scope proyek:
 * - Tree detection results   (Modul 7)
 * - Vegetation detection results (Modul 4)
 * - Land-cover classification results (Modul 8)
 *
 * Layer dengan `layer_id` yang sama akan digantikan (replace)
 * jika command dipanggil ulang.
 */
export function useSpatialResultLayers() {
  const map = useMap();
  const layersRef = useRef<Map<string, L.LayerGroup>>(new Map());
  const [activeLayers, setActiveLayers] = React.useState<LayerPayload[]>([]);

  useEffect(() => {
    let cancelled = false;

    invokeMap<MapLayerCommand[]>("get_dummy_spatial_layers")
      .then((commands) => {
        if (cancelled) return;
        const loadedLayers: LayerPayload[] = [];

        // Urutkan layer agar poligon di bawah (1), garis di tengah (2), dan titik pohon paling atas (3)
        const sortedCommands = [...commands].sort((a, b) => {
          const getScore = (cmd: MapLayerCommand) => {
            if (cmd.payload.display_preference.type === "image_overlay") return 0;
            const hasPoint = cmd.payload.spatial_results.some((r) => r.geometry.type === "Point");
            if (hasPoint) return 3;
            const hasLine = cmd.payload.spatial_results.some(
              (r) => r.geometry.type === "LineString" || r.geometry.type === "MultiLineString",
            );
            if (hasLine) return 2;
            return 1; // Polygon
          };
          return getScore(a) - getScore(b);
        });

        sortedCommands.forEach(({ action, payload }) => {
          if (action !== "load_spatial_result") return;

          const existing = layersRef.current.get(payload.layer_id);
          if (existing) map.removeLayer(existing);

          const group = buildLayerGroup(payload);
          group.addTo(map);
          layersRef.current.set(payload.layer_id, group);
          loadedLayers.push(payload);
        });
        setActiveLayers(loadedLayers);
      })
      .catch((err) => console.error("Gagal memuat dummy spatial layers:", err));

    const currentLayers = layersRef.current;

    return () => {
      cancelled = true;
      currentLayers.forEach((group) => map.removeLayer(group));
      currentLayers.clear();
      setActiveLayers([]);
    };
  }, [map]);

  return { activeLayers };
}
// Created by EnBee (Naufal Rifqi Rahman)
