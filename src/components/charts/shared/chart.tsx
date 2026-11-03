// chart.tsx

import type { ReactNode } from "react";

import "../style.css";
import { ChartFrame, type ChartSize } from "./frame";
import { Legend, type LegendItem } from "./legend";

export interface ChartProps {
  readonly empty?: boolean;
  readonly legend?: readonly LegendItem[];
  readonly children: (size: ChartSize) => ReactNode;
}

export function Chart({ empty, legend, children }: ChartProps) {
  if (empty) {
    return (
      <div className="chart">
        <div className="chart_empty">No data</div>
      </div>
    );
  }
  return (
    <div className="chart">
      <ChartFrame>{children}</ChartFrame>
      {legend && legend.length > 0 ? <Legend items={legend} /> : null}
    </div>
  );
}
