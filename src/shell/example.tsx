// example.tsx

import type { ReactNode } from "react";

import { Area } from "../components/charts/area";
import { Bar } from "../components/charts/bar";
import { Donut } from "../components/charts/donut";
import { Line } from "../components/charts/line";
import { Pie } from "../components/charts/pie";
import { StackedBar } from "../components/charts/stacked-bar";
import { Stick } from "../components/charts/stick";
import { Wave } from "../components/charts/wave";

interface ExampleItem {
  readonly id: string;
  readonly title: string;
  readonly path: string;
  readonly preview: ReactNode;
  readonly wide?: boolean;
}

interface ExampleSection {
  readonly id: string;
  readonly title: string;
  readonly items: readonly ExampleItem[];
}

const LAND_COVER = [
  { label: "Forest", value: 4200 },
  { label: "Cropland", value: 3100 },
  { label: "Water", value: 1800 },
  { label: "Urban", value: 1250 },
  { label: "Barren", value: 650 },
];

const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

const FLIGHTS = [12, 19, 14, 22, 28, 31, 26, 34, 29, 21, 17, 15].map((value, index) => ({
  label: MONTHS[index],
  value,
}));

const SITES = [
  { label: "Site A", value: 82 },
  { label: "Site B", value: 64 },
  { label: "Site C", value: 91 },
  { label: "Site D", value: 47 },
  { label: "Site E", value: 73 },
  { label: "Site F", value: 58 },
];

const PIPELINE = {
  labels: ["Jan", "Feb", "Mar", "Apr", "May", "Jun"],
  series: [
    { name: "Processed", values: [120, 150, 170, 140, 190, 210] },
    { name: "Queued", values: [40, 30, 55, 45, 35, 50] },
    { name: "Failed", values: [8, 12, 6, 10, 5, 7] },
  ],
};

const NDVI = {
  labels: MONTHS,
  series: [
    {
      name: "Zone A",
      values: [0.31, 0.34, 0.42, 0.55, 0.68, 0.74, 0.78, 0.72, 0.6, 0.48, 0.38, 0.33],
    },
    {
      name: "Zone B",
      values: [0.22, 0.25, 0.3, 0.41, 0.52, 0.6, 0.64, 0.58, 0.47, 0.36, 0.28, 0.24],
    },
  ],
};

const COVERAGE = {
  labels: MONTHS,
  series: [
    {
      name: "Captured",
      values: [210, 260, 320, 410, 520, 610, 640, 600, 520, 430, 330, 260],
    },
    {
      name: "Processed",
      values: [150, 210, 270, 360, 450, 540, 590, 550, 470, 380, 280, 220],
    },
  ],
};

const SECTIONS: readonly ExampleSection[] = [
  {
    id: "charts",
    title: "Charts",
    items: [
      {
        id: "pie",
        title: "Pie",
        path: "src/components/charts/pie.tsx",
        preview: <Pie slices={LAND_COVER} />,
      },
      {
        id: "donut",
        title: "Donut",
        path: "src/components/charts/donut.tsx",
        preview: <Donut slices={LAND_COVER} />,
      },
      {
        id: "bar",
        title: "Bar",
        path: "src/components/charts/bar.tsx",
        preview: <Bar data={FLIGHTS} />,
      },
      {
        id: "stacked-bar",
        title: "Stacked Bar",
        path: "src/components/charts/stacked-bar.tsx",
        preview: <StackedBar labels={PIPELINE.labels} series={PIPELINE.series} />,
      },
      {
        id: "stick",
        title: "Stick",
        path: "src/components/charts/stick.tsx",
        preview: <Stick data={SITES} />,
      },
      {
        id: "line",
        title: "Line",
        path: "src/components/charts/line.tsx",
        preview: <Line labels={NDVI.labels} series={NDVI.series} />,
      },
      {
        id: "wave",
        title: "Wave",
        path: "src/components/charts/wave.tsx",
        preview: <Wave labels={NDVI.labels} series={NDVI.series} />,
        wide: true,
      },
      {
        id: "area",
        title: "Area",
        path: "src/components/charts/area.tsx",
        preview: <Area labels={COVERAGE.labels} series={COVERAGE.series} />,
        wide: true,
      },
    ],
  },
];

export function Example() {
  return (
    <div className="example">
      <h1 className="example_title">Examples</h1>
      {SECTIONS.map((section) => (
        <section key={section.id} className="example_section">
          <h2 className="example_section-title">{section.title}</h2>
          <div className="example_grid">
            {section.items.map((item) => (
              <article
                key={item.id}
                className={`example_card${item.wide ? " example_card--wide" : ""}`}
              >
                <header className="example_card-header">
                  <span className="example_card-title">{item.title}</span>
                  <code className="example_card-path">{item.path}</code>
                </header>
                <div className="example_card-body">{item.preview}</div>
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
