import type { DroneImageMetadata } from "../types/marker";

const METERS_PER_DEG_LAT = 111_320;

/** Assumed horizontal field of view of the mapping camera (no sensor data in DB). */
const ASSUMED_HFOV_DEG = 60;

/** Fallback height/width ratio when the row has no dimensions (4:3). */
const DEFAULT_ASPECT = 0.75;

/**
 * Footprint for one flight photo.
 *
 * The database stores no sensor data (no focal length, no yaw), so the
 * footprint is an estimate: `width_m = 2 · altitude · tan(HFOV/2)`, with
 * height from the pixel aspect ratio. Axis-aligned because heading is
 * unknown. Neighbouring frames overlap heavily — that is how mapping surveys
 * are flown — and the overlap is kept on purpose: shrinking frames to tile
 * edge to edge would misplace analysis results on the map.
 *
 * Null when altitude or position is missing.
 */
export function estimatePhotoBounds(
  image: DroneImageMetadata,
): [[number, number], [number, number]] | null {
  const alt = image.altitude_agl;
  const hasPosition = Number.isFinite(image.latitude) && Number.isFinite(image.longitude);
  if (!hasPosition || !Number.isFinite(alt) || alt <= 0) return null;

  const widthM = 2 * alt * Math.tan(((ASSUMED_HFOV_DEG / 2) * Math.PI) / 180);
  const aspect =
    image.width && image.height && image.width > 0 ? image.height / image.width : DEFAULT_ASPECT;
  const heightM = widthM * aspect;

  const dLat = heightM / METERS_PER_DEG_LAT;
  const dLon = widthM / (METERS_PER_DEG_LAT * Math.cos((image.latitude * Math.PI) / 180));

  return [
    [image.latitude - dLat / 2, image.longitude - dLon / 2],
    [image.latitude + dLat / 2, image.longitude + dLon / 2],
  ];
}
