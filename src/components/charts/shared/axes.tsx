// axes.tsx

import type { Plot } from "./geometry";
import { formatCompact, labelStep, type Scale } from "./utils";

export interface AxesProps {
  readonly plot: Plot;
  readonly scale: Scale;
  readonly yAt: (value: number) => number;
  readonly labels: readonly string[];
  readonly band: number;
}

export function Axes({ plot, scale, yAt, labels, band }: AxesProps) {
  const step = labelStep(labels, band);
  return (
    <g>
      {scale.ticks.map((tick) => (
        <g key={tick}>
          <line
            x1={plot.left}
            x2={plot.left + plot.width}
            y1={yAt(tick)}
            y2={yAt(tick)}
            className={tick === 0 ? "chart_axis" : "chart_grid"}
          />
          <text
            x={plot.left - 8}
            y={yAt(tick)}
            textAnchor="end"
            dominantBaseline="middle"
            className="chart_tick"
          >
            {formatCompact(tick)}
          </text>
        </g>
      ))}
      {labels.map((label, index) =>
        index % step === 0 ? (
          <text
            key={`${index}-${label}`}
            x={plot.left + band * (index + 0.5)}
            y={plot.top + plot.height + 18}
            textAnchor="middle"
            className="chart_tick"
          >
            {label}
          </text>
        ) : null,
      )}
    </g>
  );
}
