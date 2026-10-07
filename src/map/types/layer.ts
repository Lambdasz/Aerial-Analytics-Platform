export type LayerState = {
  layer_id: string;
  layer_name: string;
  category: string;
  is_visible: boolean;
  opacity: number;
  z_index: number;
};

export type AnnotationFeature = {
  annotation_id: string;
  text: string;
  geometry: SpatialGeometry;
  created_at: string;
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
