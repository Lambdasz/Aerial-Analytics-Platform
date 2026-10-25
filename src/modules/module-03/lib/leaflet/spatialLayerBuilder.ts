import * as L from "leaflet";
import type { LayerPayload, SpatialResult, LayerDisplayMode } from "../../types/map";
import { buildPopupHtml } from "../popupHtml";
import { estimatePolygonArea } from "../geoJson";

/**
 * Mengubah path / URL citra overlay agar dapat ditampilkan secara reliabel oleh browser / Leaflet.
 */
export function resolveImageOverlayUrl(rawUrl: string): string {
  if (!rawUrl) return rawUrl;
  const trimmed = rawUrl.trim().replace(/\\/g, "/");

  // Jika memuat map_storage path, ekstrak menjadi path web '/map_storage/...'
  const idx = trimmed.indexOf("map_storage/");
  if (idx !== -1) {
    return `/${trimmed.substring(idx)}`;
  }

  if (!/^(https?:\/\/|data:|\/|asset:)/i.test(trimmed)) {
    return `/${trimmed}`;
  }

  return trimmed;
}

/**
 * Menyusun URL citra orthomosaic yang dapat dirender oleh browser (JPG / PNG).
 * Jika thumbnail_url tidak ada dan fallback ke file_path berkas asli (yang mungkin berupa .tif/.tiff),
 * fungsi ini TIDAK akan menyusun 'display.tif' (karena browser tidak bisa merender TIFF),
 * melainkan mengarahkannya secara presisi ke 'display.jpg' atau 'display.png' pada direktori yang sama.
 */
export function resolveOrthoDisplayUrl(img: {
  id?: string;
  thumbnail_url?: string;
  file_path?: string;
  extension?: string;
}): string | null {
  if (img.thumbnail_url && !/\.(tif|tiff)$/i.test(img.thumbnail_url)) {
    return resolveImageOverlayUrl(img.thumbnail_url);
  }

  if (img.file_path) {
    const raw = img.file_path.trim().replace(/\\/g, "/");
    if (/\.(tif|tiff)$/i.test(raw)) {
      const lastSlash = raw.lastIndexOf("/");
      const baseDir = lastSlash !== -1 ? raw.substring(0, lastSlash) : "";
      // Gunakan display.png jika ekstensi citra png, selain itu default ke display.jpg
      const displayExt = img.extension?.toLowerCase() === "png" ? "png" : "jpg";
      return resolveImageOverlayUrl(`${baseDir}/display.${displayExt}`);
    }
    return resolveImageOverlayUrl(raw);
  }

  if (img.id) {
    const displayExt = img.extension?.toLowerCase() === "png" ? "png" : "jpg";
    return resolveImageOverlayUrl(`/map_storage/images/${img.id}/display.${displayExt}`);
  }

  return null;
}

/**
 * Memvalidasi apakah koordinat bounding box berada dalam rentang WGS84 derajat sah (-90..90, -180..180).
 * Mencegah browser / Leaflet membeku bila menerima koordinat proyeksi UTM (dalam meter).
 */
export function isValidWgs84Bounds(
  bounds?: [[number, number], [number, number]] | number[][],
): boolean {
  if (!bounds || bounds.length !== 2) return false;
  const [sw, ne] = bounds;
  if (!sw || !ne || sw.length !== 2 || ne.length !== 2) return false;
  const [south, west] = sw;
  const [north, east] = ne;
  if (
    typeof south !== "number" ||
    typeof west !== "number" ||
    typeof north !== "number" ||
    typeof east !== "number" ||
    isNaN(south) ||
    isNaN(west) ||
    isNaN(north) ||
    isNaN(east)
  ) {
    return false;
  }
  return (
    south >= -90 &&
    south <= 90 &&
    north >= -90 &&
    north <= 90 &&
    west >= -180 &&
    west <= 180 &&
    east >= -180 &&
    east <= 180 &&
    south <= north
  );
}

/**
 * Mengambil nilai luasan fitur dalam meter persegi (m²) dari properti atau geometri.
 */
export function resolveFeatureArea(
  properties: Record<string, unknown>,
  coordinates?: number[][][],
): number {
  const candidateKeys = [
    "area_sqm",
    "affected_area_sqm",
    "area",
    "area_m2",
    "calculated_area_sq_m",
    "luas",
  ];
  for (const key of candidateKeys) {
    const val = properties[key];
    if (typeof val === "number" && !Number.isNaN(val) && val > 0) {
      return val;
    }
  }
  return estimatePolygonArea(coordinates);
}

/**
 * Mengambil nilai luasan MultiPolygon dalam meter persegi (m²).
 */
export function resolveMultiPolygonArea(
  properties: Record<string, unknown>,
  coordinates?: number[][][][],
): number {
  const candidateKeys = [
    "area_sqm",
    "affected_area_sqm",
    "area",
    "area_m2",
    "calculated_area_sq_m",
    "luas",
  ];
  for (const key of candidateKeys) {
    const val = properties[key];
    if (typeof val === "number" && !Number.isNaN(val) && val > 0) {
      return val;
    }
  }
  if (!coordinates || coordinates.length === 0) return 0;
  return coordinates.reduce((sum, polyCoords) => sum + estimatePolygonArea(polyCoords), 0);
}

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
    if (!isValidWgs84Bounds(display.bounds)) {
      console.warn(
        "Bounds citra overlay bukan WGS84 yang valid (mis. UTM), dilewati:",
        display.bounds,
      );
      return null;
    }
    const resolvedUrl = resolveImageOverlayUrl(display.image_url);
    return L.imageOverlay(resolvedUrl, display.bounds, {
      opacity: display.opacity,
      interactive: true,
      pane: "orthoPane",
    });
  }

  switch (result.geometry.type) {
    case "Point": {
      const [lng, lat] = result.geometry.coordinates;
      if (display.type === "point") {
        if (display.icon === "tree_marker") {
          const icon = L.divIcon({
            className: "aap-tree-marker",
            html: `<svg width="24" height="24" viewBox="0 0 24 24" fill="${display.color}" stroke="#ffffff" stroke-width="1.5"><path d="M12 2L4 12h5v8h6v-8h5z"/></svg>`,
            iconSize: [24, 24],
            iconAnchor: [12, 24],
            popupAnchor: [0, -24],
          });
          return L.marker([lat, lng], { icon });
        }

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
      const color = "color" in display ? display.color : "#E74C3C";
      const weight = display.type === "line" ? display.weight || 3 : 3;
      const polyline = L.polyline(latlngs, { color, weight });
      (polyline as unknown as { _isLine: boolean })._isLine = true;
      return polyline;
    }
    case "MultiLineString": {
      const lines = result.geometry.coordinates.map((line) =>
        line.map(([lng, lat]) => [lat, lng] as [number, number]),
      );
      const color = "color" in display ? display.color : "#E74C3C";
      const weight = display.type === "line" ? display.weight || 3 : 3;
      const polyline = L.polyline(lines, { color, weight });
      (polyline as unknown as { _isLine: boolean })._isLine = true;
      return polyline;
    }
    case "Polygon": {
      const rings = result.geometry.coordinates.map((ring) =>
        ring.map(([lng, lat]) => [lat, lng] as [number, number]),
      );
      const color = "color" in display ? display.color : "#3388ff";
      const opacity = "opacity" in display ? display.opacity : 0.25;
      const poly = L.polygon(rings, {
        color,
        weight: 2,
        fillOpacity: opacity,
      });
      const area = resolveFeatureArea(result.properties, result.geometry.coordinates);
      (poly as unknown as { _calculatedArea: number })._calculatedArea = area;
      return poly;
    }
    case "MultiPolygon": {
      const polys = result.geometry.coordinates.map((poly) =>
        poly.map((ring) => ring.map(([lng, lat]) => [lat, lng] as [number, number])),
      );
      const color = "color" in display ? display.color : "#3388ff";
      const opacity = "opacity" in display ? display.opacity : 0.25;
      const poly = L.polygon(polys, {
        color,
        weight: 2,
        fillOpacity: opacity,
      });
      const area = resolveMultiPolygonArea(result.properties, result.geometry.coordinates);
      (poly as unknown as { _calculatedArea: number })._calculatedArea = area;
      return poly;
    }
    default:
      return null;
  }
}

export interface LayerGroupBuildResult {
  group: L.LayerGroup;
  polygonGroup: L.LayerGroup;
  pointGroup: L.LayerGroup;
  pointLayers: L.Layer[];
}

/**
 * Membangun L.LayerGroup serta mengumpulkan referensi seluruh sub-layer titik (Point)
 * dan sub-layer area/poligon secara terpisah untuk mendukung filter LOD berbasis geometri.
 * Mengatur hierarki z-order otomatis: Poligon terluas di bawah, area dalam (lebih kecil)
 * di atas, garis di tengah, dan titik/dot di paling atas.
 */
export function buildLayerGroupWithPoints(
  payload: LayerPayload,
  initialPointVisible: boolean = true,
): LayerGroupBuildResult {
  const group = L.layerGroup();
  const polygonGroup = L.layerGroup();
  const pointGroup = L.layerGroup();
  const pointLayers: L.Layer[] = [];

  if (payload.display_preference.type === "image_overlay") {
    if (isValidWgs84Bounds(payload.display_preference.bounds)) {
      const resolvedUrl = resolveImageOverlayUrl(payload.display_preference.image_url);
      const layer = L.imageOverlay(resolvedUrl, payload.display_preference.bounds, {
        opacity: payload.display_preference.opacity,
        interactive: true,
        pane: "orthoPane",
      });
      if (payload.spatial_results && payload.spatial_results.length > 0) {
        layer.bindPopup(
          buildPopupHtml(
            payload.spatial_results[0],
            payload.layer_name,
            payload.display_preference.image_url,
            payload.display_preference.fields,
          ),
        );
      }
      group.addLayer(layer);
      polygonGroup.addLayer(layer);
    } else {
      console.warn(
        "Bounds image overlay bukan WGS84 yang sah (mis. UTM), dilewati:",
        payload.display_preference.bounds,
      );
    }
  } else {
    // Urutkan fitur spasial:
    // 1. Poligon terluas ditaruh di paling bawah
    // 2. Poligon yang lebih kecil (area dalam / nested) di atasnya
    // 3. Garis (LineString)
    // 4. Titik (Point)
    const sortedResults = [...payload.spatial_results].sort((a, b) => {
      const typeRank = (t: string) =>
        t === "Polygon" || t === "MultiPolygon"
          ? 1
          : t === "LineString" || t === "MultiLineString"
            ? 2
            : 3;
      const rankA = typeRank(a.geometry.type);
      const rankB = typeRank(b.geometry.type);
      if (rankA !== rankB) return rankA - rankB;

      if (
        (a.geometry.type === "Polygon" || a.geometry.type === "MultiPolygon") &&
        (b.geometry.type === "Polygon" || b.geometry.type === "MultiPolygon")
      ) {
        const areaA =
          a.geometry.type === "Polygon"
            ? resolveFeatureArea(a.properties, a.geometry.coordinates)
            : resolveMultiPolygonArea(a.properties, a.geometry.coordinates);
        const areaB =
          b.geometry.type === "Polygon"
            ? resolveFeatureArea(b.properties, b.geometry.coordinates)
            : resolveMultiPolygonArea(b.properties, b.geometry.coordinates);
        // Area terbesar lebih dulu (bawah), area terkecil belakangan (atas)
        return areaB - areaA;
      }
      return 0;
    });

    const layerImageUrl =
      "image_url" in payload.display_preference ? payload.display_preference.image_url : undefined;
    const fieldsDef =
      "fields" in payload.display_preference ? payload.display_preference.fields : undefined;
    const displayColor =
      "color" in payload.display_preference ? payload.display_preference.color : undefined;

    sortedResults.forEach((result) => {
      const layer = buildFeatureLayer(result, payload.display_preference);
      if (layer) {
        layer.bindPopup(
          buildPopupHtml(result, payload.layer_name, layerImageUrl, fieldsDef, displayColor),
        );
        if (result.geometry.type === "Point") {
          pointLayers.push(layer);
          pointGroup.addLayer(layer);
          if (initialPointVisible) {
            group.addLayer(layer);
          }
        } else {
          polygonGroup.addLayer(layer);
          group.addLayer(layer);
        }
      }
    });
  }

  return { group, polygonGroup, pointGroup, pointLayers };
}

/**
 * Membangun L.LayerGroup yang memuat seluruh fitur dari suatu LayerPayload.
 *
 * Deterministik, membuat objek mutable Leaflet (L.layerGroup).
 */
export function buildLayerGroup(payload: LayerPayload): L.LayerGroup {
  return buildLayerGroupWithPoints(payload, true).group;
}
