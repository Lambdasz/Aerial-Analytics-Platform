// wave.tsx

import { SeriesChart } from "./shared/series";
import type { SeriesChartProps } from "./shared/utils";

export function Wave({ labels, series }: SeriesChartProps) {
  return <SeriesChart labels={labels} series={series} smooth fill includeZero />;
}
