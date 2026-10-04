// stacked-bar.tsx

import { StackedChart } from "./shared/stacked";
import type { SeriesChartProps } from "./shared/utils";

export function StackedBar({ labels, series }: SeriesChartProps) {
  return <StackedChart labels={labels} series={series} />;
}
