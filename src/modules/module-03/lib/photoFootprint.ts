import type { DroneImageMetadata } from "../types/marker";

const METERS_PER_DEG_LAT = 111_320;

/**
 * Assumed horizontal field of view of the mapping camera.
 *
 * The DB stores no sensor data (no focal length, no yaw) — footprint width is
 * therefore an estimate: `width_m = 2 · altitude · tan(HFOV/2)`. At the
 * session's 405 m altitude this gives ~469 m across, in the right ballpark
 * for a DJI-class mapping frame. Footprints are axis-aligned: heading is
 * unknown, so rotated rectangles are impossible.
 */
const ASSUMED_HFOV_DEG = 60;

/** Fallback height/width ratio when the row has no dimensions (4:3). */
const DEFAULT_ASPECT = 0.75;

/**
 * Estimates an axis-aligned WGS84 footprint for one flight photo from its
 * GPS point, altitude, and pixel aspect ratio. Null when altitude is missing —
 * a zero-size rectangle would render nothing useful anyway.
 */
export function estimatePhotoBounds(
  image: DroneImageMetadata,
): [[number, number], [number, number]] | null {
  const alt = image.altitude_agl;
  if (!Number.isFinite(alt) || alt <= 0) return null;

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
