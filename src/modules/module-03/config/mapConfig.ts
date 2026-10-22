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

export const BASEMAPS = {
  osm: {
    name: "OpenStreetMap",
    url: "https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png",
    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
    maxNativeZoom: 19,
  },
  satellite: {
    name: "Esri Satellite",
    url: "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{z}/{y}/{x}",
    attribution:
      "Tiles &copy; Esri &mdash; Source: Esri, i-cubed, USDA, USGS, AEX, GeoEye, Getmapping, Aerogrid, IGN, IGP, UPR-EGP, and the GIS User Community",
    maxNativeZoom: 18,
  },
  positron: {
    name: "CartoDB Positron",
    url: `https://{s}.basemaps.cartocdn.com/light_all/{z}/{x}/{y}{r}.png?key=${
      import.meta.env.VITE_CARTO_API_KEY || ""
    }`,
    attribution:
      '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors &copy; <a href="https://carto.com/attributions">CARTO</a>',
    maxNativeZoom: 19,
  },
} as const;

export type BasemapKey = keyof typeof BASEMAPS;
