import type { DroneImageMetadata } from "../types/marker";

const METERS_PER_DEG_LAT = 111_320;

/** Assumed HFOV — fallback footprint when no session grid exists (<2 frames). */
const ASSUMED_HFOV_DEG = 60;

/** Fallback height/width ratio when the row has no dimensions (4:3). */
const DEFAULT_ASPECT = 0.75;

/**
 * Session grid: the exclusive per-frame ground cell, derived from real image
 * spacing (median along-track step × median line spacing). Cells tile edge to
 * edge, so frames never stack — unlike the true 469 m FOV footprint, which
 * overlaps ~6× because mapping surveys fly 75%/57% overlap by design.
 */
export interface FlightGrid {
  /** Median distance between consecutive frames on one flight line. */
  cellHeightM: number;
  /** Median cross-track spacing between flight lines; equals cellHeightM on a single line. */
  cellWidthM: number;
}

/** New flight line starts when sorted longitudes gap by more than ~45 m. */
const LINE_SPLIT_LON_DEG = 0.0005;

const median = (values: number[]): number => {
  const sorted = [...values].sort((a, b) => a - b);
  const mid = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 1 ? sorted[mid] : (sorted[mid - 1] + sorted[mid]) / 2;
};

const isValidPosition = (m: DroneImageMetadata): boolean =>
  Number.isFinite(m.latitude) && Number.isFinite(m.longitude);

const metersBetween = (dLat: number, dLon: number, atLat: number): number =>
  Math.hypot(
    dLat * METERS_PER_DEG_LAT,
    dLon * METERS_PER_DEG_LAT * Math.cos((atLat * Math.PI) / 180),
  );

/**
 * Derives the session's frame grid from image positions.
 *
 * Flight lines are found by clustering marker longitudes (gap >
 * `LINE_SPLIT_LON_DEG`), so marker order does not matter; lines are assumed to
 * run roughly north–south. Returns null with fewer than two valid frames.
 */
export function computeFlightGrid(markers: ReadonlyArray<DroneImageMetadata>): FlightGrid | null {
  const valid = markers.filter(isValidPosition);
  if (valid.length < 2) return null;

  const sorted = [...valid].sort((a, b) => a.longitude - b.longitude);
  const clusters: DroneImageMetadata[][] = [[sorted[0]]];
  for (let i = 1; i < sorted.length; i++) {
    const current = clusters[clusters.length - 1];
    if (sorted[i].longitude - current[current.length - 1].longitude > LINE_SPLIT_LON_DEG) {
      clusters.push([sorted[i]]);
    } else {
      current.push(sorted[i]);
    }
  }

  const lineOf = new Map<DroneImageMetadata, number>();
  clusters.forEach((cluster, index) => cluster.forEach((m) => lineOf.set(m, index)));

  // Along-track steps: consecutive frames that share a line, in flight order.
  const alongSteps: number[] = [];
  for (let i = 1; i < valid.length; i++) {
    const prev = valid[i - 1];
    const cur = valid[i];
    if (lineOf.get(prev) !== lineOf.get(cur)) continue;
    const step = metersBetween(
      cur.latitude - prev.latitude,
      cur.longitude - prev.longitude,
      prev.latitude,
    );
    if (step > 0.5) alongSteps.push(step);
  }
  if (alongSteps.length === 0) return null;

  const centers = clusters
    .map((cluster) => ({
      lat: cluster.reduce((sum, m) => sum + m.latitude, 0) / cluster.length,
      lon: cluster.reduce((sum, m) => sum + m.longitude, 0) / cluster.length,
    }))
    .sort((a, b) => a.lon - b.lon);
  // Cross-track spacing is longitude-only: the along-track drift between line
  // centres (mean latitude differs per line) would otherwise inflate the cell.
  const lineSteps: number[] = [];
  for (let i = 1; i < centers.length; i++) {
    const dLon = Math.abs(centers[i].lon - centers[i - 1].lon);
    const step = dLon * METERS_PER_DEG_LAT * Math.cos((centers[i].lat * Math.PI) / 180);
    if (step > 0.5) lineSteps.push(step);
  }

  const cellHeightM = median(alongSteps);
  return { cellHeightM, cellWidthM: lineSteps.length > 0 ? median(lineSteps) : cellHeightM };
}

/**
 * Footprint for one flight photo. With the session `grid` the cell is a
 * spacing-sized rectangle centered on the GPS point (no overlap). Without a
 * grid (single frame) falls back to the estimated FOV rectangle.
 * Null when the position is invalid, or altitude is missing on the fallback.
 */
export function estimatePhotoBounds(
  image: DroneImageMetadata,
  grid: FlightGrid | null,
): [[number, number], [number, number]] | null {
  if (!isValidPosition(image)) return null;

  let widthM: number;
  let heightM: number;
  if (grid) {
    widthM = grid.cellWidthM;
    heightM = grid.cellHeightM;
  } else {
    const alt = image.altitude_agl;
    if (!Number.isFinite(alt) || alt <= 0) return null;
    widthM = 2 * alt * Math.tan(((ASSUMED_HFOV_DEG / 2) * Math.PI) / 180);
    const aspect =
      image.width && image.height && image.width > 0 ? image.height / image.width : DEFAULT_ASPECT;
    heightM = widthM * aspect;
  }

  const dLat = heightM / METERS_PER_DEG_LAT;
  const dLon = widthM / (METERS_PER_DEG_LAT * Math.cos((image.latitude * Math.PI) / 180));

  return [
    [image.latitude - dLat / 2, image.longitude - dLon / 2],
    [image.latitude + dLat / 2, image.longitude + dLon / 2],
  ];
}
