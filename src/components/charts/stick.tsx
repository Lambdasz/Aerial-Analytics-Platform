// stick.tsx

import { ColumnChart } from "./shared/columns";
import type { Datum } from "./shared/utils";

export interface StickProps {
  readonly data: readonly Datum[];
  readonly color?: string;
}

export function Stick({ data, color }: StickProps) {
  return <ColumnChart data={data} color={color} variant="stick" />;
}

/*
SAMPLE USAGE
If chart not showing, give its parents some height/widht

import { Stick } from "../../components/charts/stick";

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
        <Stick data={flights} />
      </div>
      <div style={{ height: 320 }}>
        <Stick data={flights} color="#e0a95f" />
      </div>
      <div style={{ height: 320 }}>
        <Stick data={flightsColored} />
      </div>
    </>
  );
}
*/
