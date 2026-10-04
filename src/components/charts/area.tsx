// area.tsx

import { SeriesChart } from "./shared/series";
import type { SeriesChartProps } from "./shared/utils";

export function Area({ labels, series }: SeriesChartProps) {
  return <SeriesChart labels={labels} series={series} smooth={false} fill includeZero />;
}
