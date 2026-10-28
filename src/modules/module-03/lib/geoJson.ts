import type { Polygon } from "../types/aoi";

/**
 * Mengonversi array cincin koordinat (LatLng) menjadi struktur GeoJSON Polygon (RFC 7946).
 *
 * Koordinat Leaflet menggunakan `[lat, lng]`; GeoJSON menggunakan `[lng, lat]`.
 * Fungsi ini murni deterministik dan memastikan setiap ring tertutup.
 */
export function latLngsToPolygon(rawRings: unknown): Polygon {
  if (!rawRings || !Array.isArray(rawRings) || rawRings.length === 0) {
    return [];
  }

  // Normalisasi ke bentuk Array<Array<{ lat: number; lng: number }>>
  let rings: Array<Array<{ lat: number; lng: number }>> = [];
  const first = rawRings[0] as unknown;

  if (first && typeof first === "object" && "lat" in first && "lng" in first) {
    // 1D: [ { lat, lng }, ... ]
    rings = [rawRings as Array<{ lat: number; lng: number }>];
  } else if (Array.isArray(first)) {
    const firstNested = first[0] as unknown;
    if (
      firstNested &&
      typeof firstNested === "object" &&
      "lat" in firstNested &&
      "lng" in firstNested
    ) {
      // 2D: [ [ { lat, lng }, ... ] ]
      rings = rawRings as Array<Array<{ lat: number; lng: number }>>;
    } else if (Array.isArray(firstNested)) {
      // 3D: [ [ [ { lat, lng }, ... ] ] ] (MultiPolygon outer ring)
      rings = first as Array<Array<{ lat: number; lng: number }>>;
    }
  }

  return rings.map((ring) => {
    const coords = ring.map((ll): [number, number] => [ll.lng, ll.lat]);
    // Pastikan cincin tertutup (persyaratan RFC 7946)
    if (coords.length > 0) {
      const firstCoord = coords[0];
      const lastCoord = coords[coords.length - 1];
      if (firstCoord[0] !== lastCoord[0] || firstCoord[1] !== lastCoord[1]) {
        coords.push([...firstCoord]);
      }
    }
    return coords;
  });
}

/**
 * Menghitung estimasi luas poligon GeoJSON dalam meter persegi (m²).
 * Menggunakan perhitungan geodesik metrik agar sebanding dengan properti area_sqm / luas riil.
 * Digunakan untuk mengurutkan layer area yang bertumpuk (area terbesar di bawah, area terkecil di atas).
 */
export function estimatePolygonArea(coordinates?: number[][][]): number {
  if (!coordinates || coordinates.length === 0 || !coordinates[0] || coordinates[0].length < 3) {
    return 0;
  }
  return calculateGeodesicAreaMeters(coordinates[0] as [number, number][]);
}

/**
 * Mengonversi array titik koordinat (LatLng) Leaflet Polyline menjadi struktur GeoJSON LineString coordinates.
 * Leaflet: [lat, lng], GeoJSON: [lng, lat].
 */
export function latLngsToLineString(rawPoints: unknown): [number, number][] {
  if (!rawPoints || !Array.isArray(rawPoints) || rawPoints.length === 0) {
    return [];
  }
  const points = Array.isArray(rawPoints[0]) ? (rawPoints[0] as unknown[]) : rawPoints;
  const result: [number, number][] = [];
  for (const pt of points) {
    if (pt && typeof pt === "object" && "lat" in pt && "lng" in pt) {
      const ll = pt as { lat: number; lng: number };
      result.push([Number(ll.lng), Number(ll.lat)]);
    } else if (Array.isArray(pt) && pt.length >= 2) {
      result.push([Number(pt[1]), Number(pt[0])]);
    }
  }
  return result;
}

/**
 * Menghitung estimasi panjang garis dalam meter menggunakan Haversine.
 */
export function calculateLineLengthMeters(coords: [number, number][]): number {
  if (coords.length < 2) return 0;
  let total = 0;
  const R = 6371000;
  for (let i = 0; i < coords.length - 1; i++) {
    const [lon1, lat1] = coords[i];
    const [lon2, lat2] = coords[i + 1];
    const phi1 = (lat1 * Math.PI) / 180;
    const phi2 = (lat2 * Math.PI) / 180;
    const deltaPhi = ((lat2 - lat1) * Math.PI) / 180;
    const deltaLambda = ((lon2 - lon1) * Math.PI) / 180;
    const a =
      Math.sin(deltaPhi / 2) * Math.sin(deltaPhi / 2) +
      Math.cos(phi1) * Math.cos(phi2) * Math.sin(deltaLambda / 2) * Math.sin(deltaLambda / 2);
    const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));
    total += R * c;
  }
  return total;
}

/**
 * Menghitung luas area poligon dalam meter persegi (m²) dengan proyeksi lokal ekuator.
 */
export function calculateGeodesicAreaMeters(ring: [number, number][]): number {
  if (ring.length < 3) return 0;
  const avgLat = ring.reduce((acc, c) => acc + c[1], 0) / ring.length;
  const metersPerDegreeLat = 111139;
  const metersPerDegreeLon = 111139 * Math.cos((avgLat * Math.PI) / 180);
  let area = 0;
  for (let i = 0; i < ring.length - 1; i++) {
    const [x1, y1] = ring[i];
    const [x2, y2] = ring[i + 1];
    const mx1 = x1 * metersPerDegreeLon;
    const my1 = y1 * metersPerDegreeLat;
    const mx2 = x2 * metersPerDegreeLon;
    const my2 = y2 * metersPerDegreeLat;
    area += mx1 * my2 - mx2 * my1;
  }
  return Math.abs(area) / 2;
}

/**
 * Mengonversi teks multi-baris koordinat desimal "lat, lng" atau "lat lng" menjadi array [lng, lat] GeoJSON.
 */
export function parseCoordinatesText(text: string): [number, number][] {
  const lines = text.split("\n");
  const coords: [number, number][] = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const parts = trimmed.split(/[\s,;]+/).filter(Boolean);
    if (parts.length >= 2) {
      const lat = parseFloat(parts[0]);
      const lng = parseFloat(parts[1]);
      if (
        !Number.isNaN(lat) &&
        !Number.isNaN(lng) &&
        lat >= -90 &&
        lat <= 90 &&
        lng >= -180 &&
        lng <= 180
      ) {
        coords.push([lng, lat]);
      }
    }
  }
  return coords;
}

/**
 * Format luasan area dalam meter persegi atau hektar.
 */
export function formatArea(sqm: number): string {
  if (sqm >= 10_000) {
    return `${(sqm / 10_000).toFixed(2)} ha`;
  }
  return `${sqm.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m²`;
}

/**
 * Format keliling dalam meter atau kilometer.
 */
export function formatPerimeter(meters: number): string {
  if (meters >= 1_000) {
    return `${(meters / 1_000).toFixed(2)} km`;
  }
  return `${meters.toLocaleString("id-ID", { maximumFractionDigits: 1 })} m`;
}
