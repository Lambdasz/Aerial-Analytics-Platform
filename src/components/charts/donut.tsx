// donut.tsx

import { RadialChart } from "./shared/radial";
import type { Datum } from "./shared/utils";

export interface DonutProps {
  readonly slices: readonly Datum[];
}

export function Donut({ slices }: DonutProps) {
  return <RadialChart slices={slices} inner={0.62} />;
}
