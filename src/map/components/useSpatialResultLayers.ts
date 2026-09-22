import { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import { invoke } from "@tauri-apps/api/core";
import * as L from "leaflet";
import type {
  MapLayerCommand,
  LayerPayload,
  SpatialResult,
  LayerDisplayMode,
} from "../types/spatial_result";

function buildFeatureLayer(result: SpatialResult, display: LayerDisplayMode): L.Layer | null {
  switch (result.geometry.type) {
    case "Point": {
      const [lng, lat] = result.geometry.coordinates; // GeoJSON -> Leaflet
      if (display.type === "point") {
        return L.circleMarker([lat, lng], { color: display.color, radius: 8 });
      }
      return L.marker([lat, lng]);
    }
    case "Polygon": {
      const rings = result.geometry.coordinates.map((ring) =>
        ring.map(([lng, lat]) => [lat, lng] as [number, number]),
      );
      // Belum ada display_preference khusus polygon di kontrak backend;
      // fallback warna default sampai kontraknya ditambah.
      const color = display.type === "point" ? display.color : "#3388ff";
      return L.polygon(rings, { color });
    }
    default:
      return null;
  }
}

function buildPopupHtml(result: SpatialResult): string {
  const rows = Object.entries(result.properties)
    .map(([key, value]) => `<div><strong>${key}</strong>: ${value}</div>`)
    .join("");
  return `<div class="spatial-result-popup">${rows}</div>`;
}

function buildLayerGroup(payload: LayerPayload): L.LayerGroup {
  const group = L.layerGroup();
  payload.spatial_results.forEach((result) => {
    const layer = buildFeatureLayer(result, payload.display_preference);
    if (layer) {
      layer.bindPopup(buildPopupHtml(result));
      group.addLayer(layer);
    }
  });
  return group;
}

/**
 * Memuat data dummy hasil spasial dari backend Rust (command
 * `get_dummy_spatial_layers`) dan merender tiap layer (tree/vegetation/
 * building) ke peta Leaflet. Layer dengan `layer_id` yang sama akan
 * digantikan (replace) jika command dipanggil ulang.
 */
export function useSpatialResultLayers() {
  const map = useMap();
  const layersRef = useRef<Map<string, L.LayerGroup>>(new Map());

  useEffect(() => {
    let cancelled = false;

    invoke<MapLayerCommand[]>("get_dummy_spatial_layers")
      .then((commands) => {
        if (cancelled) return;
        commands.forEach(({ action, payload }) => {
          if (action !== "load_spatial_result") return;

          const existing = layersRef.current.get(payload.layer_id);
          if (existing) map.removeLayer(existing);

          const group = buildLayerGroup(payload);
          group.addTo(map);
          layersRef.current.set(payload.layer_id, group);
        });
      })
      .catch((err) => console.error("Gagal memuat dummy spatial layers:", err));

    return () => {
      cancelled = true;
      layersRef.current.forEach((group) => map.removeLayer(group));
      layersRef.current.clear();
    };
  }, [map]);
}
