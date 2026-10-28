/**
 * Kontrak tipe domain untuk spatial results (Bagian 12 & 13).
 */

export type FieldKind = "text" | "number" | "boolean";

export interface FieldDef {
  key: string;
  label: string;
  unit?: string | null;
  visible: boolean; // default true
  custom: boolean; // default false
  kind?: FieldKind;
}

export interface FeatureEdit {
  index: number;
  set?: Record<string, unknown>;
  remove?: string[];
}

export interface SpatialResultSummary {
  id: string;
  layer_name: string;
  feature_count: number;
  geom_type: string; // "Point" | "Polygon" | "Line" | "ImageOverlay" | "Mixed"
  display_color: string;
  icon?: string | null;
  thumbnail_url?: string | null;
  format_type: "flat" | "decomposed";
  is_corrupted: boolean;
  updated_at?: string | null;
  has_manual_edits: boolean;
  concurrency_token: string;
}

export interface UpdateSpatialResultPayload {
  id: string;
  name?: string;
  display_color?: string;
  icon?: string;
  image_action?: "keep" | "delete" | "replace";
  image_path?: string;
  fields?: FieldDef[];
  feature_edits?: FeatureEdit[];
  expected_updated_at?: string;
}

export interface DeleteSpatialResultSummary {
  id: string;
  deleted_from_disk: boolean;
  layers_removed: number;
  feature_count: number;
  photos_count: number;
}

export interface ManualEditsCheckResult {
  id: string;
  has_manual_edits: boolean;
  custom_fields_count: number;
  edited_features_count: number;
}
