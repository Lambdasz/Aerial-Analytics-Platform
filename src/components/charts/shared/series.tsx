// series.tsx

import { useId } from "react";

import { Axes } from "./axes";
import { Chart } from "./chart";
import type { ChartSize } from "./frame";
import { areaPath, linePath, plotArea, type Point } from "./geometry";
import { useColumnHover } from "./hover";
import { Tooltip } from "./tooltip";
import { colorAt, formatFull, niceScale, ticksFor, valueAt, type SeriesChartProps } from "./utils";

export interface SeriesChartOptions {
  readonly smooth: boolean;
  readonly fill: boolean;
  readonly includeZero: boolean;
}

type Props = SeriesChartProps & SeriesChartOptions;

export function SeriesChart(props: Props) {
  const { labels, series } = props;
  const legend =
    series.length > 1
      ? series.map((item, index) => ({
          name: item.name,
          color: colorAt(index, item.color),
        }))
      : undefined;
  return (
    <Chart empty={labels.length === 0 || series.length === 0} legend={legend}>
      {(size) => <SeriesPlot {...props} size={size} />}
    </Chart>
  );
}

function SeriesPlot({
  labels,
  series,
  smooth,
  fill,
  includeZero,
  size,
}: Props & { readonly size: ChartSize }) {
  const uid = useId().replace(/[^a-zA-Z0-9]/g, "");
  const plot = plotArea(size.width, size.height);
  const all = series.flatMap((item) => labels.map((_, index) => valueAt(item, index)));
  const scale = niceScale(Math.min(...all), Math.max(...all), includeZero, ticksFor(plot.height));
  const span = scale.max - scale.min || 1;
  const yAt = (value: number) =>
    plot.top + plot.height - ((value - scale.min) / span) * plot.height;
  const band = plot.width / labels.length;
  const xAt = (index: number) => plot.left + band * (index + 0.5);
  const baseline = yAt(scale.min <= 0 && scale.max >= 0 ? 0 : scale.min);
  const showDots = band >= 28 || labels.length === 1;
  const { hover, bind } = useColumnHover(plot, labels.length);

  const lines = series.map((item, seriesIndex) => {
    const points: Point[] = labels.map((_, index) => ({
      x: xAt(index),
      y: yAt(valueAt(item, index)),
    }));
    return { item, seriesIndex, points, color: colorAt(seriesIndex, item.color) };
  });

  return (
    <>
      <svg className="chart_svg" width={size.width} height={size.height}>
        <defs>
          {lines.map((line) => (
            <linearGradient
              key={line.seriesIndex}
              id={`${uid}-${line.seriesIndex}`}
              x1="0"
              x2="0"
              y1="0"
              y2="1"
            >
              <stop offset="0%" stopColor={line.color} stopOpacity={0.32} />
              <stop offset="100%" stopColor={line.color} stopOpacity={0.02} />
            </linearGradient>
          ))}
        </defs>
        <Axes plot={plot} scale={scale} yAt={yAt} labels={labels} band={band} />
        {hover ? (
          <line
            x1={xAt(hover.index)}
            x2={xAt(hover.index)}
            y1={plot.top}
            y2={plot.top + plot.height}
            className="chart_guide"
          />
        ) : null}
        {lines.map((line) => (
          <g key={line.seriesIndex}>
            {fill ? (
              <path
                d={areaPath(line.points, smooth, baseline)}
                fill={`url(#${uid}-${line.seriesIndex})`}
              />
            ) : null}
            <path
              d={linePath(line.points, smooth)}
              fill="none"
              stroke={line.color}
              strokeWidth={2}
              strokeLinejoin="round"
              strokeLinecap="round"
            />
            {showDots
              ? line.points.map((point, index) => (
                  <circle
                    key={index}
                    cx={point.x}
                    cy={point.y}
                    r={3.5}
                    fill="var(--chart-surface)"
                    stroke={line.color}
                    strokeWidth={2}
                  />
                ))
              : null}
            {hover ? (
              <circle
                cx={line.points[hover.index].x}
                cy={line.points[hover.index].y}
                r={5}
                fill={line.color}
                stroke="var(--chart-surface)"
                strokeWidth={2}
              />
            ) : null}
          </g>
        ))}
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
          rows={series.map((item, seriesIndex) => ({
            name: series.length > 1 ? item.name : undefined,
            color: colorAt(seriesIndex, item.color),
            value: formatFull(valueAt(item, hover.index)),
          }))}
        />
      ) : null}
    </>
  );
}
