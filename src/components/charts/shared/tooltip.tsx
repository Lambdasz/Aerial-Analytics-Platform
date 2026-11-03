// tooltip.tsx

import type { CSSProperties } from "react";

export interface TooltipRow {
  readonly name?: string;
  readonly value: string;
  readonly color?: string;
}

export interface TooltipProps {
  readonly x: number;
  readonly y: number;
  readonly frameWidth: number;
  readonly title?: string;
  readonly rows: readonly TooltipRow[];
}

export function Tooltip({ x, y, frameWidth, title, rows }: TooltipProps) {
  const flip = x > frameWidth / 2;
  const style: CSSProperties = {
    left: flip ? x - 14 : x + 14,
    top: Math.max(y, 28),
    transform: flip ? "translate(-100%, -50%)" : "translate(0, -50%)",
  };
  return (
    <div className="chart_tooltip" style={style}>
      {title ? <div className="chart_tooltip-title">{title}</div> : null}
      {rows.map((row, index) => (
        <div key={`${index}-${row.name ?? ""}`} className="chart_tooltip-row">
          {row.color ? <span className="chart_swatch" style={{ background: row.color }} /> : null}
          {row.name ? <span className="chart_tooltip-name">{row.name}</span> : null}
          <span className="chart_tooltip-value">{row.value}</span>
        </div>
      ))}
    </div>
  );
}
