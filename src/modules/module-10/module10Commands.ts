import { invoke } from "@tauri-apps/api/core";

/**
 * Typed wrappers around Module 10's Tauri commands.
 *
 * The field names mirror the Rust structs in
 * `src-tauri/src/modules/module_10/` (all serialized as camelCase), so a change
 * on the Rust side must be reflected here.
 */

/** One stored image with the metadata Module 10 needs (Module 1.4 record). */
export interface ImageObservation {
  imageId: string;
  /** Absolute path to the app-owned copy of the image. */
  filePath: string;
  /** EXIF `DateTimeOriginal`, local camera time (`2026-01-10T07:00:00`). */
  capturedAt: string;
  latitude: number | null;
  longitude: number | null;
  altitudeM: number | null;
  width: number | null;
  height: number | null;
}

/** All images captured on one calendar day. */
export interface ObservationPeriod {
  /** `YYYY-MM-DD`. */
  date: string;
  observations: ImageObservation[];
}

export type ObservationSkipReason = "missing_capture_time" | "not_analysis_ready";

/** An image Module 10 cannot use, with the reason instead of silent loss. */
export interface SkippedImage {
  imageId: string;
  filePath: string;
  reason: ObservationSkipReason;
}

/** Response of `get_observation_periods` (10.1). */
export interface SessionObservationPeriods {
  sessionId: string;
  periods: ObservationPeriod[];
  skipped: SkippedImage[];
}

/** Vegetation coverage of one image, in pixels and percent. */
export interface VegetationCoverage {
  sampledPixels: number;
  vegetationPixels: number;
  /** `0..=100`. */
  coveragePercent: number;
}

/** Percentage-point change between the two observations (10.3). */
export interface VegetationCoverageChange {
  before: number;
  after: number;
  /** `after - before`, in percentage points. */
  change: number;
}

/** Response of `get_vegetation_change` (10.2 + 10.3). */
export interface VegetationChangeReport {
  before: ImageObservation;
  after: ImageObservation;
  intervalDays: number;
  beforeCoverage: VegetationCoverage;
  afterCoverage: VegetationCoverage;
  change: VegetationCoverageChange;
}

/** 10.1 - a session's images grouped per acquisition date. */
export function getObservationPeriods(sessionId: string) {
  return invoke<SessionObservationPeriods>("get_observation_periods", { sessionId });
}

/** 10.2 + 10.3 - compare the two most recent periods of a session. */
export function getVegetationChange(sessionId: string) {
  return invoke<VegetationChangeReport>("get_vegetation_change", { sessionId });
}

// ---------------------------------------------------------------------------
// 10.4 - 10.7: per-location comparisons.
//
// The Rust commands accept `{ locationId: value }` maps produced by Module 3
// (plots / grid cells) and Module 4/5 (land-cover classification, tree
// detection). Module 10 never invents the classes or counts itself.
// ---------------------------------------------------------------------------

/** One location's land-cover class in both periods (10.4). */
export interface LandCoverChange {
  locationId: string;
  before: string;
  after: string;
  changed: boolean;
}

export type TreeCountDirection = "bertambah" | "berkurang" | "tetap";

/** One location's tree count in both periods (10.5). */
export interface TreeCountChange {
  locationId: string;
  before: number;
  after: number;
  /** `after - before`; negative when trees disappeared. */
  change: number;
  direction: TreeCountDirection;
}

export type MagnitudeLevel = "rendah" | "sedang" | "tinggi";
export type MagnitudeDirection = "meningkat" | "menurun" | "tetap";

/** One location's change magnitude (10.6). */
export interface ChangeMagnitudeEntry {
  locationId: string;
  before: number;
  after: number;
  /** Rounded to two decimals. */
  change: number;
  /** `abs(change)`, rounded to two decimals. */
  magnitude: number;
  direction: MagnitudeDirection;
  level: MagnitudeLevel;
}

/** 10.6 result plus the 10.7 flag; the Rust side flattens the fields. */
export type SignificantChangeArea = ChangeMagnitudeEntry & { significant: boolean };

/** 10.4 - compare per-location land-cover classes. */
export function getLandCoverChanges(
  beforeClasses: Record<string, string>,
  afterClasses: Record<string, string>,
) {
  return invoke<LandCoverChange[]>("get_land_cover_changes", { beforeClasses, afterClasses });
}

/** 10.5 - compare per-location tree counts. */
export function getTreeCountChanges(
  beforeCounts: Record<string, number>,
  afterCounts: Record<string, number>,
) {
  return invoke<TreeCountChange[]>("get_tree_count_changes", { beforeCounts, afterCounts });
}

/** 10.6 - per-location change, magnitude, and level. */
export function getChangeMagnitudes(
  beforeValues: Record<string, number>,
  afterValues: Record<string, number>,
) {
  return invoke<ChangeMagnitudeEntry[]>("get_change_magnitudes", { beforeValues, afterValues });
}

/** 10.7 - locations whose magnitude reaches `threshold` (default 10). */
export function getSignificantChangeAreas(
  beforeValues: Record<string, number>,
  afterValues: Record<string, number>,
  threshold?: number,
) {
  return invoke<SignificantChangeArea[]>("get_significant_change_areas", {
    beforeValues,
    afterValues,
    threshold,
  });
}

/** 10.3 cross-session comparison: baseline vs target session. */
export interface SessionComparisonReport {
  comparisonId: string;
  before: ImageObservation;
  after: ImageObservation;
  intervalDays: number;
  beforeCoverage: VegetationCoverage;
  afterCoverage: VegetationCoverage;
  change: VegetationCoverageChange;
  magnitude: number;
  magnitudeLevel: "rendah" | "sedang" | "tinggi";
}

/** One persisted comparison row (history / dashboard input). */
export interface ComparisonRecord {
  id: string;
  projectId: string | null;
  beforeSessionId: string;
  afterSessionId: string;
  beforeImageId: string;
  afterImageId: string;
  beforeDate: string;
  afterDate: string;
  intervalDays: number;
  metric: string;
  beforeValue: number;
  afterValue: number;
  delta: number;
  magnitude: number;
  magnitudeLevel: "rendah" | "sedang" | "tinggi";
  createdAt: string;
}

export function compareSessions(
  beforeSessionId: string,
  afterSessionId: string,
): Promise<SessionComparisonReport> {
  return invoke<SessionComparisonReport>("compare_sessions", {
    beforeSessionId,
    afterSessionId,
  });
}

export function getChangeSummary(projectId: string): Promise<ComparisonRecord[]> {
  return invoke<ComparisonRecord[]>("get_change_summary", { projectId });
}
