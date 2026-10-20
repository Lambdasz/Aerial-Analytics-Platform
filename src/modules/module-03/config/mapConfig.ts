/**
 * Single source of truth for Module 3 map viewport, orthomosaic bounds, and defaults.
 *
 * Geographical location: Kariangau, Balikpapan, East Kalimantan, Indonesia.
 * Orthomosaic provenance: OpenAerialMap ID 600fcc9055481e0005e312a1 (EPSG:4326).
 */

export const KARIANGAU_BOUNDS = {
  west: 116.855973140003,
  south: -1.2243110280554836,
  east: 116.863202751651,
  north: -1.2186297002884836,
} as const;

export const DEFAULT_MAP_VIEW = {
  center: [-1.22147, 116.859588] as [number, number],
  zoom: 16,
  bounds: [
    [-1.2243110280554836, 116.855973140003],
    [-1.2186297002884836, 116.863202751651],
  ] as [[number, number], [number, number]],
} as const;
