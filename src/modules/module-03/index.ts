// Komponen Publik Utama (Composition Root)
export { MapExplorer } from "./MapExplorer";
export type { MapExplorerProps } from "./MapExplorer";

// Halaman Penuh Modul
export { Module03Page } from "./pages/Module03Page";

// Toolbar AOI (Kompatibilitas Mundur)
export { AoiToolbar, AoiToolbar as AoiDemoToolbar } from "./organisms/AoiToolbar";

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
