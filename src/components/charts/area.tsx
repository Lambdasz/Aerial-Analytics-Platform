// area.tsx

import { SeriesChart } from "./shared/series";
import type { SeriesChartProps } from "./shared/utils";

export function Area({ labels, series }: SeriesChartProps) {
  return <SeriesChart labels={labels} series={series} smooth={false} fill includeZero />;
}

/*
SAMPLE USAGE
If chart not showing, give its parents some height/widht

import { Area } from "../../components/charts/area";

const labels = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];

const series = [
  { name: "Zone A", values: [0.31, 0.42, 0.55, 0.68, 0.74, 0.78] },
  { name: "Zone B", values: [0.22, 0.3, 0.41, 0.52, 0.6, 0.64] },
];

const seriesColored = [
  { name: "Zone A", values: [0.31, 0.42, 0.55, 0.68, 0.74, 0.78], color: "#4f9d69" },
  { name: "Zone B", values: [0.22, 0.3, 0.41, 0.52, 0.6, 0.64], color: "var(--chart-4)" },
];

export function VegetationPanel() {
  return (
    <>
      <div style={{ height: 320 }}>
        <Area labels={labels} series={series} />
      </div>
      <div style={{ height: 320 }}>
        <Area labels={labels} series={seriesColored} />
      </div>
    </>
  );
}
*/
