// stacked.tsx

import { Axes } from "./axes";
import { Chart } from "./chart";
import type { ChartSize } from "./frame";
import { plotArea } from "./geometry";
import { useColumnHover } from "./hover";
import { Tooltip } from "./tooltip";
import { colorAt, formatFull, niceScale, ticksFor, valueAt, type SeriesChartProps } from "./utils";

export function StackedChart({ labels, series }: SeriesChartProps) {
  const legend = series.map((item, index) => ({
    name: item.name,
    color: colorAt(index, item.color),
  }));
  return (
    <Chart empty={labels.length === 0 || series.length === 0} legend={legend}>
      {(size) => <StackedPlot labels={labels} series={series} size={size} />}
    </Chart>
  );
}

function StackedPlot({ labels, series, size }: SeriesChartProps & { readonly size: ChartSize }) {
  const plot = plotArea(size.width, size.height);
  const totals = labels.map((_, index) =>
    series.reduce((sum, item) => sum + Math.max(0, valueAt(item, index)), 0),
  );
  const scale = niceScale(0, Math.max(...totals), true, ticksFor(plot.height));
  const span = scale.max - scale.min || 1;
  const yAt = (value: number) =>
    plot.top + plot.height - ((value - scale.min) / span) * plot.height;
  const band = plot.width / labels.length;
  const thickness = Math.max(1, Math.min(56, band * 0.62));
  const { hover, bind } = useColumnHover(plot, labels.length);

  return (
    <>
      <svg className="chart_svg" width={size.width} height={size.height}>
        <Axes plot={plot} scale={scale} yAt={yAt} labels={labels} band={band} />
        {hover ? (
          <rect
            x={plot.left + band * hover.index}
            y={plot.top}
            width={band}
            height={plot.height}
            className="chart_hover-band"
          />
        ) : null}
        {labels.map((label, index) => {
          let accumulated = 0;
          return (
            <g key={`${index}-${label}`}>
              {series.map((item, seriesIndex) => {
                const value = Math.max(0, valueAt(item, index));
                const bottom = yAt(accumulated);
                const top = yAt(accumulated + value);
                accumulated += value;
                return (
                  <rect
                    key={`${seriesIndex}-${item.name}`}
                    x={plot.left + band * (index + 0.5) - thickness / 2}
                    y={top}
                    width={thickness}
                    height={Math.max(bottom - top, 0)}
                    fill={colorAt(seriesIndex, item.color)}
                  />
                );
              })}
            </g>
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
      {hover ? (
        <Tooltip
          x={hover.x}
          y={hover.y}
          frameWidth={size.width}
          title={labels[hover.index]}
          rows={[
            ...series.map((item, seriesIndex) => ({
              name: item.name,
              color: colorAt(seriesIndex, item.color),
              value: formatFull(valueAt(item, hover.index)),
            })),
            { name: "Total", value: formatFull(totals[hover.index]) },
          ]}
        />
      ) : null}
    </>
  );
}
