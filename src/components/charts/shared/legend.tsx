// legend.tsx

export interface LegendItem {
  readonly name: string;
  readonly color: string;
  readonly detail?: string;
}

export function Legend({ items }: { readonly items: readonly LegendItem[] }) {
  return (
    <ul className="chart_legend">
      {items.map((item, index) => (
        <li key={`${index}-${item.name}`} className="chart_legend-item">
          <span className="chart_swatch" style={{ background: item.color }} />
          <span>{item.name}</span>
          {item.detail ? <span className="chart_legend-detail">{item.detail}</span> : null}
        </li>
      ))}
    </ul>
  );
}
