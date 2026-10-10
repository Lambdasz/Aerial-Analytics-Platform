import React, { useEffect, useRef } from "react";
import { useMap } from "react-leaflet";
import * as L from "leaflet";
import { invokeMap } from "../error";
import type { MapLayerCommand, LayerPayload, SpatialResult, LayerDisplayMode } from "../types/map";

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
      const latlngs = result.geometry.coordinates.map(
        ([lng, lat]) => [lat, lng] as [number, number],
      );
      const color = display.type === "point" ? display.color : "#ff3333";
      return L.polyline(latlngs, { color });
    }
    case "MultiLineString": {
      const lines = result.geometry.coordinates.map((line) =>
        line.map(([lng, lat]) => [lat, lng] as [number, number]),
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
        <td style="color: var(--color-slate); font-weight: 600; padding: 6px 0;">${formatKey(key)}</td>
        <td style="color: var(--color-ink); font-weight: 500; text-align: right; font-family: var(--font-sf-pro-text); padding: 6px 0;">${value}</td>
      </tr>
    `,
    )
    .join("");

  return `
    <div style="
      background: var(--color-gallery-white);
      border-radius: 16px;
      padding: 16px;
      min-width: 260px;
      box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12), var(--shadow-subtle);
      font-family: var(--font-sf-pro-text);
      border: 1px solid var(--color-hairline-silver);
    ">
      <div style="font-size: 15px; font-weight: 700; color: var(--color-ink); margin-bottom: 12px; letter-spacing: -0.2px; line-height: 1.3;">
        ${layerName}
      </div>
      
      <div style="margin: 12px 0; border-bottom: 1px solid var(--color-hairline-silver);"></div>
      
      <table style="width: 100%; border-collapse: collapse; font-size: 13px;">
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
    const layer = L.imageOverlay(
      payload.display_preference.image_url,
      payload.display_preference.bounds,
      {
        opacity: payload.display_preference.opacity,
      },
    );
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
