/**
 * Module 9 — pure analytics helpers for the Plots tab.
 *
 * Functional core only: no I/O, no React, no mutation of inputs. Every
 * function returns a new value, so it can be checked without the app running.
 * Style rules: NFR-9.1.4–9.1.7 in `REQUIREMENTS.md` (branching via lookup
 * tables, one-level ternaries and `??`; collections via array HOFs).
 */
import type { PlotIndicator, PlotRecord } from "./plotCommands";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

const ERROR_READERS: ReadonlyArray<(error: unknown) => string | undefined> = [
  (error) => (typeof error === "string" ? error : undefined),
  (error) => (error instanceof Error ? error.message : undefined),
  (error) =>
    error !== null && typeof error === "object" && "message" in error
      ? String(error.message)
      : undefined,
];

/** Human-readable message from anything thrown by a Tauri command. */
export const errorMessage = (error: unknown): string =>
  ERROR_READERS.map((read) => read(error)).find((text) => text !== undefined) ??
  "Unexpected error.";

// ---------------------------------------------------------------------------
// Immutable set helpers (React state updates)
// ---------------------------------------------------------------------------

const SET_UPDATERS: Record<"add" | "remove", <T>(items: ReadonlySet<T>, item: T) => Set<T>> = {
  add: (items, item) => new Set([...items, item]),
  remove: (items, item) => new Set([...items].filter((entry) => entry !== item)),
};

/** New set with `item` included (`on = true`) or removed (`on = false`). */
export const withMember = <T>(items: ReadonlySet<T>, item: T, on: boolean): Set<T> =>
  SET_UPDATERS[on ? "add" : "remove"](items, item);

/** New set with `item` flipped. */
export const toggleMember = <T>(items: ReadonlySet<T>, item: T): Set<T> =>
  withMember(items, item, !items.has(item));

// ---------------------------------------------------------------------------
// Indicator options (FR-9.6.3)
// ---------------------------------------------------------------------------

/** One selectable indicator: the same name from two plugins is two options. */
export interface IndicatorOption {
  key: string;
  pluginId: string;
  indicator: string;
  label: string;
}

/** Stable option key for a (plugin, indicator) pair. */
export const indicatorOptionKey = (pluginId: string, indicator: string): string =>
  `${pluginId}::${indicator}`;

/** Indicator name for display (`coverage_percentage` → `coverage percentage`). */
export const humanizeIndicator = (indicator: string): string => indicator.replace(/_/g, " ");

/**
 * Numeric indicators that can be compared, one option per (plugin, indicator),
 * sorted by indicator name then plugin.
 */
export const indicatorOptions = (indicators: readonly PlotIndicator[]): IndicatorOption[] =>
  Array.from(
    new Map(
      indicators
        .filter((row) => row.value !== null)
        .map((row): [string, IndicatorOption] => [
          indicatorOptionKey(row.pluginId, row.indicator),
          {
            key: indicatorOptionKey(row.pluginId, row.indicator),
            pluginId: row.pluginId,
            indicator: row.indicator,
            label: `${humanizeIndicator(row.indicator)} · ${row.pluginId}`,
          },
        ]),
    ).values(),
  ).sort((a, b) => a.indicator.localeCompare(b.indicator) || a.pluginId.localeCompare(b.pluginId));

// ---------------------------------------------------------------------------
// Comparison and ranking (FR-9.5, FR-9.6)
// ---------------------------------------------------------------------------

/** One plot's value for the compared indicator. */
export interface ComparisonEntry {
  plot: PlotRecord;
  value: number;
  unit: string | null;
}

/**
 * Values of one (plugin, indicator) for every plot that has it. Plots without
 * a value are left out — never treated as 0 (FR-9.5.2).
 */
export const comparisonEntries = (
  plots: readonly PlotRecord[],
  indicators: readonly PlotIndicator[],
  option: Pick<IndicatorOption, "pluginId" | "indicator"> | null,
): ComparisonEntry[] =>
  plots.flatMap((plot) =>
    indicators
      .filter(
        (row) =>
          row.plotId === plot.id &&
          row.pluginId === option?.pluginId &&
          row.indicator === option?.indicator &&
          row.value !== null,
      )
      .slice(0, 1)
      .map((row) => ({ plot, value: row.value as number, unit: row.unit })),
  );

export type SortDirection = "desc" | "asc";

/** Indicators where a smaller value is better; ranked lowest first by default. */
export const LOWER_IS_BETTER: ReadonlySet<string> = new Set([
  "yellowing_pct",
  "browning_pct",
  "moderate_stress_pct",
  "severe_stress_pct",
]);

/** Default ranking direction for an indicator (FR-9.6.2). */
export const defaultDirection = (indicator: string): SortDirection =>
  LOWER_IS_BETTER.has(indicator) ? "asc" : "desc";

const COMPARATORS: Record<SortDirection, (a: ComparisonEntry, b: ComparisonEntry) => number> = {
  desc: (a, b) => b.value - a.value,
  asc: (a, b) => a.value - b.value,
};

/** Ranked copy of `entries`; equal values keep their input order (stable sort). */
export const rankEntries =
  (direction: SortDirection) =>
  (entries: readonly ComparisonEntry[]): ComparisonEntry[] =>
    [...entries].sort(COMPARATORS[direction]);

// ---------------------------------------------------------------------------
// Statistics (FR-9.4.2)
// ---------------------------------------------------------------------------

/** Summary statistics of one indicator across plots. */
export interface IndicatorStats {
  count: number;
  mean: number;
  min: number;
  max: number;
  /** Population standard deviation (0 for a single value). */
  stdDev: number;
}

/** Stats over the finite numbers in `values`; `null` entries are ignored. `null` when none. */
export const indicatorStats = (values: readonly (number | null)[]): IndicatorStats | null => {
  const numbers = values.filter(
    (value): value is number => value !== null && Number.isFinite(value),
  );
  const count = numbers.length;
  const mean = numbers.reduce((sum, value) => sum + value, 0) / Math.max(count, 1);
  const variance =
    numbers.reduce((sum, value) => sum + (value - mean) ** 2, 0) / Math.max(count, 1);
  return count === 0
    ? null
    : {
        count,
        mean,
        min: Math.min(...numbers),
        max: Math.max(...numbers),
        stdDev: Math.sqrt(variance),
      };
};

// ---------------------------------------------------------------------------
// Summary (FR-9.7)
// ---------------------------------------------------------------------------

/** One plugin's latest indicators for a plot. */
export interface PluginIndicatorGroup {
  pluginId: string;
  rows: PlotIndicator[];
}

/** Latest indicators of one plot, grouped per plugin (first-seen order). */
export const indicatorsByPlugin = (
  indicators: readonly PlotIndicator[],
  plotId: string,
): PluginIndicatorGroup[] => {
  const own = indicators.filter((row) => row.plotId === plotId);
  return Array.from(new Set(own.map((row) => row.pluginId))).map((pluginId) => ({
    pluginId,
    rows: own.filter((row) => row.pluginId === pluginId),
  }));
};
