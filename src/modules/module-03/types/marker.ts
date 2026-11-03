export interface DroneImageMetadata {
  image_id: string;
  file_name: string;
  latitude: number;
  longitude: number;
  altitude_agl: number;
  heading_deg: number;
  thumbnail_url: string;
  /** Library path — lets the popup lazily fetch a thumbnail via `thumbnail_image`. */
  file_path?: string;
  /** Recorded file size, part of the thumbnail cache identity. */
  file_size?: number | null;
  /** Pixel dimensions — used for the footprint aspect ratio. */
  width?: number | null;
  height?: number | null;
}
