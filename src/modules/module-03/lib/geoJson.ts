import type { Polygon } from "../types/aoi";

/**
 * Mengonversi array cincin koordinat (LatLng) menjadi struktur GeoJSON Polygon (RFC 7946).
 *
 * Koordinat Leaflet menggunakan `[lat, lng]`; GeoJSON menggunakan `[lng, lat]`.
 * Fungsi ini murni deterministik dan memastikan setiap ring tertutup.
 */
export function latLngsToPolygon(rings: Array<Array<{ lat: number; lng: number }>>): Polygon {
  return rings.map((ring) => {
    const coords = ring.map((ll): [number, number] => [ll.lng, ll.lat]);
    // Pastikan cincin tertutup (persyaratan RFC 7946)
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
