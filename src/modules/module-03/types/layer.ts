export type LayerState = {
  layer_id: string;
  layer_name: string;
  category: string;
  is_visible: boolean;
  opacity: number;
  z_index: number;
  source_type?: string;
  source_id?: string;
};

export type AnnotationFeature = {
  id: string;
  schema_version?: number;
  text: string;
  geometry: SpatialGeometry;
  created_at: string;
  /** Backward compatibility alias */
  annotation_id?: string;
};

export type SpatialGeometry =
  | {
      type: "Point";
      coordinates: [number, number];
    }
  | {
      type: "Polygon";
      coordinates: number[][][];
    }
  | {
      type: "LineString";
      coordinates: [number, number][];
    };
