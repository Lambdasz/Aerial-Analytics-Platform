/**
 * Kontrak tipe data penyimpanan citra (Module 3 Storage).
 */

export type ImageCategory = "ortho" | "raster";

export interface ImportImageParams {
  filePath: string;
  west: number;
  south: number;
  east: number;
  north: number;
}

export interface StoredImageMetadata {
  schema_version?: number;
  id?: string;
  filename: string;
  category: ImageCategory;
  extension?: string;
  format?: string;
  file_path?: string;
  size_bytes: number;
  dimensions?: [number, number];
  resolution_px?: [number, number];
  thumbnail_url?: string;
  bounds?: [[number, number], [number, number]];
  aspect_ratio?: number;
  display_size_bytes?: number;
  created_at?: string;
  imported_at?: string;
  updated_at?: string;
  is_corrupted?: boolean;
}

export interface UpdateOrthoImageParams {
  id: string;
  filename?: string;
  west?: number;
  south?: number;
  east?: number;
  north?: number;
}

export interface DeleteOrthoSummary {
  id: string;
  image_deleted: boolean;
  layer_deleted: boolean;
  annotations_deleted_count: number;
}
