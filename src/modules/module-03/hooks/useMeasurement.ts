/**
 * Hook for spatial measurement tools (distance, area, perimeter).
 *
 * Manages measurement mode state and delegates computation to the backend
 * via {@link measureDistance}, {@link measureArea}, and
 * {@link measurePerimeter} from `api.ts`.
 *
 * ## Design
 *
 * - Only one measurement mode is active at a time.
 * - Results accumulate in a list so users can compare successive measurements.
 * - Coordinates are `[longitude, latitude]` (GeoJSON RFC 7946). The caller
 *   (Person 1's component) is responsible for converting Leaflet `LatLng` to
 *   this format before passing to the measurement functions.
 *
 * @module
 */

import { useCallback, useState } from "react";

import { measureArea, measureDistance, measurePerimeter } from "../api";
import type { Coordinate, MeasurementResult, Polygon } from "../types/aoi";

/** Measurement modes, or `null` when idle. */
export type MeasurementMode = "distance" | "area" | "perimeter" | null;

/** Public API surface returned by {@link useMeasurement}. */
export interface UseMeasurementReturn {
  /** Currently active measurement mode (`null` = idle). */
  mode: MeasurementMode;
  /** Whether a measurement call is in flight. */
  isMeasuring: boolean;
  /** Accumulated measurement results (most recent first). */
  results: MeasurementResult[];
  /** Enter distance measurement mode. */
  startMeasureDistance: () => void;
  /** Enter area measurement mode. */
  startMeasureArea: () => void;
  /** Enter perimeter measurement mode. */
  startMeasurePerimeter: () => void;
  /** Exit the current measurement mode without discarding results. */
  stopMeasurement: () => void;
  /** Clear all accumulated results. */
  clearResults: () => void;
  /**
   * Execute a distance measurement between two points.
   *
   * @param pointA — `[longitude, latitude]` of the first point.
   * @param pointB — `[longitude, latitude]` of the second point.
   * @returns Distance in meters.
   */
  doMeasureDistance: (pointA: Coordinate, pointB: Coordinate) => Promise<number>;
  /**
   * Execute an area measurement for a polygon.
   *
   * @param polygon — GeoJSON-style polygon rings.
   * @returns Area in square meters.
   */
  doMeasureArea: (polygon: Polygon) => Promise<number>;
  /**
   * Execute a perimeter measurement for a polygon.
   *
   * @param polygon — GeoJSON-style polygon rings.
   * @returns Perimeter in meters.
   */
  doMeasurePerimeter: (polygon: Polygon) => Promise<number>;
}

/**
 * React hook for spatial measurement operations.
 *
 * @example
 * ```tsx
 * function MeasureToolbar() {
 *   const { mode, startMeasureDistance, doMeasureDistance, results } = useMeasurement();
 *
 *   return (
 *     <>
 *       <button onClick={startMeasureDistance}>📏 Distance</button>
 *       {results.map((r, i) => (
 *         <div key={i}>{r.mode}: {r.value.toFixed(2)}</div>
 *       ))}
 *     </>
 *   );
 * }
 * ```
 */
export function useMeasurement(): UseMeasurementReturn {
  const [mode, setMode] = useState<MeasurementMode>(null);
  const [isMeasuring, setIsMeasuring] = useState(false);
  const [results, setResults] = useState<MeasurementResult[]>([]);

  // ── Mode switching ─────────────────────────────────────────────────

  const startMeasureDistance = useCallback(() => setMode("distance"), []);
  const startMeasureArea = useCallback(() => setMode("area"), []);
  const startMeasurePerimeter = useCallback(() => setMode("perimeter"), []);
  const stopMeasurement = useCallback(() => setMode(null), []);
  const clearResults = useCallback(() => setResults([]), []);

  // ── Measurement execution ──────────────────────────────────────────

  const appendResult = useCallback((entry: MeasurementResult) => {
    setResults((prev) => [entry, ...prev]);
  }, []);

  const doMeasureDistance = useCallback(
    async (pointA: Coordinate, pointB: Coordinate): Promise<number> => {
      setIsMeasuring(true);
      try {
        const value = await measureDistance(pointA, pointB);
        appendResult({
          mode: "distance",
          value,
          timestamp: new Date().toISOString(),
        });
        return value;
      } finally {
        setIsMeasuring(false);
      }
    },
    [appendResult],
  );

  const doMeasureArea = useCallback(
    async (polygon: Polygon): Promise<number> => {
      setIsMeasuring(true);
      try {
        const value = await measureArea(polygon);
        appendResult({
          mode: "area",
          value,
          timestamp: new Date().toISOString(),
        });
        return value;
      } finally {
        setIsMeasuring(false);
      }
    },
    [appendResult],
  );

  const doMeasurePerimeter = useCallback(
    async (polygon: Polygon): Promise<number> => {
      setIsMeasuring(true);
      try {
        const value = await measurePerimeter(polygon);
        appendResult({
          mode: "perimeter",
          value,
          timestamp: new Date().toISOString(),
        });
        return value;
      } finally {
        setIsMeasuring(false);
      }
    },
    [appendResult],
  );

  return {
    mode,
    isMeasuring,
    results,
    startMeasureDistance,
    startMeasureArea,
    startMeasurePerimeter,
    stopMeasurement,
    clearResults,
    doMeasureDistance,
    doMeasureArea,
    doMeasurePerimeter,
  };
}
