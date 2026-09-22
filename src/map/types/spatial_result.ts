export interface MapLayerCommand {
  action: string;
  payload: LayerPayload;
}

export interface LayerPayload {
  layer_id: string;
  layer_name: string;
  display_preference: LayerDisplayMode;
  spatial_results: SpatialResult[];
}

export type LayerDisplayMode =
  | { type: "image_overlay"; image_url: string; opacity: number }
  | { type: "point"; color: string; icon: string };

export interface SpatialResult {
  source: string;
  geometry: SpatialGeometry;
  properties: SpatialProperties;
}

// Catatan: coordinates mengikuti urutan GeoJSON [longitude, latitude]
export type SpatialGeometry =
  { type: "Point"; coordinates: [number, number] } | { type: "Polygon"; coordinates: number[][][] };

export type SpatialProperties = TreeProperties | VegetationProperties | BuildingProperties;

export interface TreeProperties {
  tree_id: string;
  confidence: number;
  height_est_m: number;
}
export interface VegetationProperties {
  vegetation_index_type: string;
  mean_greenness_score: number;
  area_sqm: number;
}
export interface BuildingProperties {
  building_id: string;
}
