import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { invokeMap } from "../error";
import type {
  MapLayerCommand,
  LayerPayload,
  SpatialResult,
  LayerDisplayMode,
} from "../types/map";

function buildFeatureLayer(result: SpatialResult, display: LayerDisplayMode): L.Layer | null {
  if (display.type === "image_overlay") {
    const imageOverlay = L.imageOverlay(display.image_url, display.bounds, {
      opacity: display.opacity,
    });
    return imageOverlay;
  }

  switch (result.geometry.type) {
    case "Point": {
      const [lng, lat] = result.geometry.coordinates; // GeoJSON -> Leaflet
      if (display.type === "point") {
        return L.circleMarker([lat, lng], { color: display.color, radius: 8 });
      }
      return L.marker([lat, lng]);
    }
    case "LineString": {
      const latlngs = result.geometry.coordinates.map(([lng, lat]) => [lat, lng] as [number, number]);
      const color = display.type === "point" ? display.color : "#ff3333";
      return L.polyline(latlngs, { color });
    }
    case "MultiLineString": {
      const lines = result.geometry.coordinates.map((line) =>
        line.map(([lng, lat]) => [lat, lng] as [number, number])
      );
      const color = display.type === "point" ? display.color : "#ff3333";
      return L.polyline(lines, { color });
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

function buildPopupHtml(result: SpatialResult, layerName: string): string {
  const formatKey = (k: string) => k.replace(/_/g, " ").replace(/\b\w/g, (c) => c.toUpperCase());

  const rows = Object.entries(result.properties)
    .map(
      ([key, value]) => `
      <tr>
        <td class="bp5-text-muted"><strong>${formatKey(key)}</strong></td>
        <td>${value}</td>
      </tr>
    `,
    )
    .join("");

  return `
    <div class="bp5-card bp5-elevation-2 spatial-result-popup-card" style="padding: 10px; min-width: 220px; background-color: rgba(255, 255, 255, 0.95); border-radius: 8px; box-shadow: 0 4px 12px rgba(0,0,0,0.15);">
      <h6 class="bp5-heading" style="margin-top: 0; margin-bottom: 10px; color: #2C3E50; font-size: 14px; border-bottom: 1px solid #ddd; padding-bottom: 8px;">
        ${layerName}
      </h6>
      <table class="bp5-html-table bp5-html-table-condensed bp5-html-table-striped" style="width: 100%; margin: 0; font-size: 13px; color: #1f2937; background: transparent;">
        <tbody>
          ${rows}
        </tbody>
      </table>
    </div>
  `;
}

function buildLayerGroup(payload: LayerPayload): L.LayerGroup {
  const group = L.layerGroup();
  
  if (payload.display_preference.type === "image_overlay") {
    // SpatialResult is not needed for image_overlay, but the function signature expects one.
    // However, since we bypassed it earlier, let's just create it directly here
    const layer = L.imageOverlay(payload.display_preference.image_url, payload.display_preference.bounds, {
      opacity: payload.display_preference.opacity,
    });
    group.addLayer(layer);
  } else {
    payload.spatial_results.forEach((result) => {
      const layer = buildFeatureLayer(result, payload.display_preference);
      if (layer) {
        layer.bindPopup(buildPopupHtml(result, payload.layer_name));
        group.addLayer(layer);
      }
    });
  }

  return group;
}

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
        commands.forEach(({ action, payload }) => {
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