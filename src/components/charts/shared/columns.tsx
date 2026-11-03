// columns.tsx

import { Axes } from "./axes";
import { Chart } from "./chart";
import type { ChartSize } from "./frame";
import { plotArea } from "./geometry";
import { useColumnHover } from "./hover";
import { Tooltip } from "./tooltip";
import { PALETTE, formatFull, niceScale, ticksFor, type Datum } from "./utils";

export type ColumnVariant = "bar" | "stick";

export interface ColumnChartProps {
  readonly data: readonly Datum[];
  readonly color?: string;
  readonly variant: ColumnVariant;
}

export function ColumnChart({ data, color, variant }: ColumnChartProps) {
  return (
    <Chart empty={data.length === 0}>
      {(size) => <ColumnPlot data={data} color={color} variant={variant} size={size} />}
    </Chart>
  );
}

function ColumnPlot({
  data,
  color,
  variant,
  size,
}: ColumnChartProps & { readonly size: ChartSize }) {
  const plot = plotArea(size.width, size.height);
  const values = data.map((datum) => datum.value);
  const scale = niceScale(Math.min(...values), Math.max(...values), true, ticksFor(plot.height));
  const span = scale.max - scale.min || 1;
  const yAt = (value: number) =>
    plot.top + plot.height - ((value - scale.min) / span) * plot.height;
  const band = plot.width / data.length;
  const zero = yAt(0);
  const thickness = Math.max(1, Math.min(56, band * 0.62));
  const dot = Math.max(2.5, Math.min(6, band / 4));
  const { hover, bind } = useColumnHover(plot, data.length);
  const active = hover ? data[hover.index] : null;

  return (
    <>
      <svg className="chart_svg" width={size.width} height={size.height}>
        <Axes
          plot={plot}
          scale={scale}
          yAt={yAt}
          labels={data.map((datum) => datum.label)}
          band={band}
        />
        {hover ? (
          <rect
            x={plot.left + band * hover.index}
            y={plot.top}
            width={band}
            height={plot.height}
            className="chart_hover-band"
          />
        ) : null}
        {data.map((datum, index) => {
          const fill = datum.color ?? color ?? PALETTE[0];
          const center = plot.left + band * (index + 0.5);
          const top = yAt(datum.value);
          if (variant === "stick") {
            return (
              <g key={`${index}-${datum.label}`}>
                <line
                  x1={center}
                  x2={center}
                  y1={zero}
                  y2={top}
                  stroke={fill}
                  strokeWidth={2}
                  strokeLinecap="round"
                />
                <circle cx={center} cy={top} r={dot} fill={fill} />
              </g>
            );
          }
          return (
            <rect
              key={`${index}-${datum.label}`}
              x={center - thickness / 2}
              y={Math.min(top, zero)}
              width={thickness}
              height={Math.max(Math.abs(top - zero), 1)}
              rx={Math.min(4, thickness / 2)}
              fill={fill}
            />
          );
        })}
        <rect
          x={plot.left}
          y={plot.top}
          width={plot.width}
          height={plot.height}
          fill="transparent"
          {...bind}
        />
      </svg>
      {hover && active ? (
        <Tooltip
          x={hover.x}
          y={hover.y}
          frameWidth={size.width}
          title={active.label}
          rows={[
            {
              color: active.color ?? color ?? PALETTE[0],
              value: formatFull(active.value),
            },
          ]}
        />
      ) : null}
    </>
  );
}
