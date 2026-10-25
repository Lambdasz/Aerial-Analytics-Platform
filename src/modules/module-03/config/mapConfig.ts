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

/**
 * Batas ukuran berkas citra orthomosaic: maksimal 50 MB (52.428.800 bytes).
 */
export const MAX_IMAGE_FILE_SIZE_BYTES = 50 * 1024 * 1024;

/**
 * Batas dimensi citra orthomosaic: maksimal 16.384 piksel per sisi.
 */
export const MAX_IMAGE_DIMENSION_PX = 16384;

/**
 * Batas maksimal ukuran berkas pratinjau display citra orthomosaic: 5 MB (5.242.880 bytes).
 * Citra yang melebihi batas ini tidak akan dimuat ke DOM Leaflet demi performa dan stabilitas memori.
 */
export const MAX_DISPLAY_FILE_SIZE_BYTES = 5 * 1024 * 1024;

/**
 * Level zoom minimum untuk me-render citra orthomosaic drone di atas peta (zoom >= 16).
 * Jika pengguna zoom out di bawah level 16 (zoom < 16), citra tidak akan dimuat/ditampilkan.
 */
export const MIN_ORTHO_ZOOM = 16;

export const DEFAULT_MAP_VIEW = {
  center: [-1.22147, 116.859588] as [number, number],
  zoom: 16,
  bounds: [
    [-1.2243110280554836, 116.855973140003],
    [-1.2186297002884836, 116.863202751651],
  ] as [[number, number], [number, number]],
} as const;

/**
 * Level zoom minimum untuk menampilkan fitur area (Poligon, Persegi/Square, AOI, klasifikasi tutupan lahan).
 * Di bawah level ini (zoom < 15), layer poligon area disembunyikan agar tampilan overview tetap bersih.
 */
export const MIN_POLYGON_FEATURE_ZOOM = 15;

/**
 * Level zoom minimum untuk menampilkan fitur titik detail (Point, Dot, Titik, Penanda Kecil, Deteksi Pohon, Anotasi Titik, Marker Foto Drone).
 * Di bawah level ini (zoom < 17), seluruh penanda titik detail otomatis disembunyikan.
 */
export const MIN_POINT_FEATURE_ZOOM = 17;

export const MIN_FEATURE_ZOOM = MIN_POLYGON_FEATURE_ZOOM;

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
} as const;

export type BasemapKey = keyof typeof BASEMAPS;
