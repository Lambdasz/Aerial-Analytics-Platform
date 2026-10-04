// line.tsx

import { SeriesChart } from "./shared/series";
import type { SeriesChartProps } from "./shared/utils";

export function Line({ labels, series }: SeriesChartProps) {
  return (
    <SeriesChart labels={labels} series={series} smooth={false} fill={false} includeZero={false} />
  );
}
