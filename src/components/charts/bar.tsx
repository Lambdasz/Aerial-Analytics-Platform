// bar.tsx

import { ColumnChart } from "./shared/columns";
import type { Datum } from "./shared/utils";

export interface BarProps {
  readonly data: readonly Datum[];
  readonly color?: string;
}

export function Bar({ data, color }: BarProps) {
  return <ColumnChart data={data} color={color} variant="bar" />;
}

/*
SAMPLE USAGE
If chart not showing, give its parents some height/widht

import { Bar } from "../../components/charts/bar";

const flights = [
  { label: "Jan", value: 12 },
  { label: "Feb", value: 19 },
  { label: "Mar", value: 14 },
  { label: "Apr", value: 22 },
];

const flightsColored = [
  { label: "Jan", value: 12 },
  { label: "Feb", value: 19, color: "#d9768a" },
  { label: "Mar", value: 14 },
  { label: "Apr", value: 22, color: "var(--chart-3)" },
];

export function FlightsPanel() {
  return (
    <>
      <div style={{ height: 320 }}>
        <Bar data={flights} />
      </div>
      <div style={{ height: 320 }}>
        <Bar data={flights} color="#e0a95f" />
      </div>
      <div style={{ height: 320 }}>
        <Bar data={flightsColored} />
      </div>
    </>
  );
}
*/
