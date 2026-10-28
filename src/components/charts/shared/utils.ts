// utils.ts

export interface Datum {
  readonly label: string;
  readonly value: number;
  readonly color?: string;
}

export interface Series {
  readonly name: string;
  readonly values: readonly number[];
  readonly color?: string;
}

export interface SeriesChartProps {
  readonly labels: readonly string[];
  readonly series: readonly Series[];
}

export interface Scale {
  readonly min: number;
  readonly max: number;
  readonly ticks: readonly number[];
}

export const PALETTE: readonly string[] = [
  "var(--chart-1)",
  "var(--chart-2)",
  "var(--chart-3)",
  "var(--chart-4)",
  "var(--chart-5)",
  "var(--chart-6)",
  "var(--chart-7)",
  "var(--chart-8)",
];

export function colorAt(index: number, color?: string): string {
  return color ?? PALETTE[index % PALETTE.length];
}

const compactFormat = new Intl.NumberFormat("en", {
  notation: "compact",
  maximumFractionDigits: 2,
});

const fullFormat = new Intl.NumberFormat("en", { maximumFractionDigits: 2 });

export function formatCompact(value: number): string {
  return compactFormat.format(value);
}

export function formatFull(value: number): string {
  return fullFormat.format(value);
}

export function valueAt(series: Series, index: number): number {
  return series.values[index] ?? 0;
}

export function ticksFor(plotHeight: number): number {
  return Math.max(2, Math.min(8, Math.floor(plotHeight / 44)));
}

function niceStep(raw: number): number {
  const magnitude = Math.pow(10, Math.floor(Math.log10(raw)));
  const fraction = raw / magnitude;
  if (fraction <= 1) {
    return magnitude;
  }
  if (fraction <= 2) {
    return 2 * magnitude;
  }
  if (fraction <= 5) {
    return 5 * magnitude;
  }
  return 10 * magnitude;
}

export function niceScale(min: number, max: number, includeZero: boolean, target: number): Scale {
  let low = includeZero ? Math.min(0, min) : min;
  let high = includeZero ? Math.max(0, max) : max;
  if (low === high) {
    if (includeZero && high === 0) {
      high = 1;
    } else {
      const pad = Math.abs(low) * 0.1 || 1;
      low -= pad;
      high += pad;
    }
  }
  const step = niceStep((high - low) / Math.max(1, target));
  const niceMin = Math.floor(low / step) * step;
  const niceMax = Math.ceil(high / step) * step;
  const ticks: number[] = [];
  for (let value = niceMin; value <= niceMax + step / 2; value += step) {
    ticks.push(Number(value.toFixed(10)));
  }
  return { min: niceMin, max: niceMax, ticks };
}

export function labelStep(labels: readonly string[], band: number): number {
  const longest = labels.reduce((max, label) => Math.max(max, label.length), 0);
  const needed = longest * 6.6 + 12;
  return Math.max(1, Math.ceil(needed / Math.max(band, 1)));
}
