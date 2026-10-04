// example.tsx

import { ChartFrame } from "../components/charts/frame";

interface ExampleItem {
  readonly id: string;
  readonly title: string;
  readonly path: string;
  readonly wide?: boolean;
}

interface ExampleSection {
  readonly id: string;
  readonly title: string;
  readonly items: readonly ExampleItem[];
}

const SECTIONS: readonly ExampleSection[] = [
  {
    id: "charts",
    title: "Charts",
    items: [
      { id: "pie", title: "Pie", path: "src/components/charts/pie.tsx" },
      { id: "donut", title: "Donut", path: "src/components/charts/donut.tsx" },
      { id: "bar", title: "Bar", path: "src/components/charts/bar.tsx" },
      {
        id: "stacked-bar",
        title: "Stacked Bar",
        path: "src/components/charts/stacked-bar.tsx",
      },
      { id: "stick", title: "Stick", path: "src/components/charts/stick.tsx" },
      { id: "line", title: "Line", path: "src/components/charts/line.tsx" },
      {
        id: "wave",
        title: "Wave",
        path: "src/components/charts/wave.tsx",
        wide: true,
      },
      {
        id: "area",
        title: "Area",
        path: "src/components/charts/area.tsx",
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
                <div className="example_card-body">
                  <ChartFrame />
                </div>
              </article>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
