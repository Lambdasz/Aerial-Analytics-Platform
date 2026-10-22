import * as L from "leaflet";
import type { LayerPayload, SpatialResult, LayerDisplayMode } from "../../types/map";
import { buildPopupHtml } from "../popupHtml";

/**
 * Membangun Leaflet Layer untuk satu fitur spasial GeoJSON.
 *
 * Deterministik, membuat objek mutable Leaflet (L.circleMarker, L.polyline, L.polygon, L.imageOverlay).
 */
export function buildFeatureLayer(
  result: SpatialResult,
  display: LayerDisplayMode,
): L.Layer | null {
  if (display.type === "image_overlay") {
    return L.imageOverlay(display.image_url, display.bounds, {
      opacity: display.opacity,
    });
  }

  switch (result.geometry.type) {
    case "Point": {
      const [lng, lat] = result.geometry.coordinates;
      if (display.type === "point") {
        const marker = L.circleMarker([lat, lng], {
          pane: "markerPane",
          color: "#ffffff",
          weight: 2,
          fillColor: display.color,
          fillOpacity: 0.9,
          radius: 9,
        });

        marker.on("mouseover", () => {
          marker.bringToFront();
          marker.setStyle({ radius: 12, weight: 3 });
        });
        marker.on("mouseout", () => {
          marker.setStyle({ radius: 9, weight: 2 });
        });

        return marker;
      }
      return L.marker([lat, lng]);
    }
    case "LineString": {
      const latlngs = result.geometry.coordinates.map(
        ([lng, lat]) => [lat, lng] as [number, number],
      );
      const color = display.type === "point" ? display.color : "#ff3333";
      return L.polyline(latlngs, { color, weight: 3 });
    }
    case "MultiLineString": {
      const lines = result.geometry.coordinates.map((line) =>
        line.map(([lng, lat]) => [lat, lng] as [number, number]),
      );
      const color = display.type === "point" ? display.color : "#ff3333";
      return L.polyline(lines, { color, weight: 3 });
    }
    case "Polygon": {
      const rings = result.geometry.coordinates.map((ring) =>
        ring.map(([lng, lat]) => [lat, lng] as [number, number]),
      );
      const color = display.type === "point" ? display.color : "#3388ff";
      return L.polygon(rings, {
        color,
        weight: 2,
        fillOpacity: 0.25,
      });
    }
    default:
      return null;
  }
}

/**
 * Membangun L.LayerGroup yang memuat seluruh fitur dari suatu LayerPayload.
 *
 * Deterministik, membuat objek mutable Leaflet (L.layerGroup).
 */
export function buildLayerGroup(payload: LayerPayload): L.LayerGroup {
  const group = L.layerGroup();

  if (payload.display_preference.type === "image_overlay") {
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
