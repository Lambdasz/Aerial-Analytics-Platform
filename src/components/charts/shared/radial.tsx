// radial.tsx

import { useState } from "react";

import { Chart } from "./chart";
import type { ChartSize } from "./frame";
import { pointAt, sectorPath } from "./geometry";
import { pointerIn } from "./hover";
import { Tooltip } from "./tooltip";
import { colorAt, formatCompact, formatFull, type Datum } from "./utils";

export interface RadialChartProps {
  readonly slices: readonly Datum[];
  readonly inner: number;
}

export function RadialChart({ slices, inner }: RadialChartProps) {
  const visible = slices.filter((slice) => slice.value > 0);
  const total = visible.reduce((sum, slice) => sum + slice.value, 0);
  const legend = visible.map((slice, index) => ({
    name: slice.label,
    color: colorAt(index, slice.color),
    detail: `${Math.round((slice.value / total) * 100)}%`,
  }));
  return (
    <Chart empty={total <= 0} legend={legend}>
      {(size) => <RadialPlot slices={visible} total={total} inner={inner} size={size} />}
    </Chart>
  );
}

interface HoverState {
  readonly index: number;
  readonly x: number;
  readonly y: number;
}

const START_ANGLE = -Math.PI / 2;
const FULL_TURN = Math.PI * 2;

function RadialPlot({
  slices,
  total,
  inner,
  size,
}: RadialChartProps & { readonly total: number; readonly size: ChartSize }) {
  const [hover, setHover] = useState<HoverState | null>(null);
  const cx = size.width / 2;
  const cy = size.height / 2;
  const outer = Math.max(0, Math.min(size.width, size.height) / 2 - 6);
  const innerRadius = outer * inner;

  const arcs = slices.map((slice, index) => {
    const fraction = slice.value / total;
    const startOffset = slices.slice(0, index).reduce((sum, prior) => sum + prior.value / total, 0);
    const start = START_ANGLE + startOffset * FULL_TURN;
    const end = start + fraction * FULL_TURN;
    return { slice, index, start, end, fraction };
  });
  const active = hover ? arcs[hover.index] : null;

  return (
    <>
      <svg className="chart_svg" width={size.width} height={size.height}>
        {arcs.map((arc) => {
          const middle = (arc.start + arc.end) / 2;
          const labelAt = pointAt(cx, cy, (outer + innerRadius) / 2, middle);
          return (
            <g key={`${arc.index}-${arc.slice.label}`}>
              <path
                d={sectorPath(cx, cy, outer, innerRadius, arc.start, arc.end)}
                fill={colorAt(arc.index, arc.slice.color)}
                stroke="var(--chart-surface)"
                strokeWidth={2}
                opacity={hover && hover.index !== arc.index ? 0.55 : 1}
                onMouseMove={(event) => {
                  const pointer = pointerIn(event);
                  if (pointer) {
                    setHover({ index: arc.index, ...pointer });
                  }
                }}
                onMouseLeave={() => {
                  setHover(null);
                }}
              />
              {arc.fraction >= 0.07 && outer >= 70 ? (
                <text
                  x={labelAt.x}
                  y={labelAt.y}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  className="chart_slice-label"
                >
                  {Math.round(arc.fraction * 100)}%
                </text>
              ) : null}
            </g>
          );
        })}
        {inner > 0 && innerRadius >= 36 ? (
          <g className="chart_total">
            <text x={cx} y={cy - 4} textAnchor="middle" className="chart_total-value">
              {formatCompact(total)}
            </text>
            <text x={cx} y={cy + 14} textAnchor="middle" className="chart_total-label">
              Total
            </text>
          </g>
        ) : null}
      </svg>
      {hover && active ? (
        <Tooltip
          x={hover.x}
          y={hover.y}
          frameWidth={size.width}
          title={active.slice.label}
          rows={[
            {
              color: colorAt(active.index, active.slice.color),
              name: `${Math.round(active.fraction * 100)}%`,
              value: formatFull(active.slice.value),
            },
          ]}
        />
      ) : null}
    </>
  );
}
