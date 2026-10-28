import "./leaflet-setup";

// Komponen Publik Utama (Composition Root)
export { MapExplorer } from "./MapExplorer";
export type { MapExplorerProps } from "./MapExplorer";

// Halaman Penuh Modul
export { Module03Page } from "./pages/Module03Page";

// Toolbar & Layer AOI & Annotations
export { AoiToolbar } from "./organisms/AoiToolbar";
export { SavedAoiLayer } from "./organisms/SavedAoiLayer";
export { AnnotationLayer } from "./organisms/AnnotationLayer";
export { useAnnotations } from "./hooks/useAnnotations";
export { buildPopupHtml, buildAoiPopupHtml, buildAnnotationPopupHtml } from "./lib/popupHtml";

// Konfigurasi Zoom Threshold
export {
  MIN_POLYGON_FEATURE_ZOOM,
  MIN_POINT_FEATURE_ZOOM,
  MIN_FEATURE_ZOOM,
} from "./config/mapConfig";

// Ekspor Kontrak Tipe Spasial Publik
export type { DroneImageMetadata } from "./types/marker";
export type { LayerState, AnnotationFeature, SpatialGeometry } from "./types/layer";
export type {
  Coordinate,
  Polygon,
  AoiFeature,
  AoiProperties,
  MeasurementResult,
} from "./types/aoi";
export type {
  MapLayerCommand,
  LayerPayload,
  LayerDisplayMode,
  SpatialResult,
  SpatialProperties,
} from "./types/map";
export type { ImageCategory, StoredImageMetadata } from "./types/storage";
