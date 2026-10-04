// donut.tsx

import { RadialChart } from "./shared/radial";
import type { Datum } from "./shared/utils";

export interface DonutProps {
  readonly slices: readonly Datum[];
}

export function Donut({ slices }: DonutProps) {
  return <RadialChart slices={slices} inner={0.62} />;
}

/*
SAMPLE USAGE.
If chart not showing, give its parents some height/widht

import { Donut } from "../../components/charts/donut";

const landCover = [
  { label: "Forest", value: 4200 },
  { label: "Cropland", value: 3100 },
  { label: "Water", value: 1800 },
];

const landCoverColored = [
  { label: "Forest", value: 4200, color: "#4f9d69" },
  { label: "Cropland", value: 3100, color: "var(--chart-2)" },
  { label: "Water", value: 1800, color: "#4a90c2" },
];

export function LandCoverPanel() {
  return (
    <>
      <div style={{ height: 320 }}>
        <Donut slices={landCover} />
      </div>
      <div style={{ height: 320 }}>
        <Donut slices={landCoverColored} />
      </div>
    </>
  );
}
*/
