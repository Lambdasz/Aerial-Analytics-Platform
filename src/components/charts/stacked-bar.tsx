// stacked-bar.tsx

import { StackedChart } from "./shared/stacked";
import type { SeriesChartProps } from "./shared/utils";

export function StackedBar({ labels, series }: SeriesChartProps) {
  return <StackedChart labels={labels} series={series} />;
}

/*
SAMPLE USAGE
If chart not showing, give its parents some height/widht

import { StackedBar } from "../../components/charts/stacked-bar";

const labels = ["Jan", "Feb", "Mar", "Apr"];

const series = [
  { name: "Processed", values: [120, 150, 170, 140] },
  { name: "Queued", values: [40, 30, 55, 45] },
  { name: "Failed", values: [8, 12, 6, 10] },
];

const seriesColored = [
  { name: "Processed", values: [120, 150, 170, 140], color: "#4f9d69" },
  { name: "Queued", values: [40, 30, 55, 45], color: "var(--chart-2)" },
  { name: "Failed", values: [8, 12, 6, 10], color: "#d9768a" },
];

export function PipelinePanel() {
  return (
    <>
      <div style={{ height: 320 }}>
        <StackedBar labels={labels} series={series} />
      </div>
      <div style={{ height: 320 }}>
        <StackedBar labels={labels} series={seriesColored} />
      </div>
    </>
  );
}
*/
