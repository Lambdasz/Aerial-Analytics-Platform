// pie.tsx

import { RadialChart } from "./shared/radial";
import type { Datum } from "./shared/utils";

export interface PieProps {
  readonly slices: readonly Datum[];
}

export function Pie({ slices }: PieProps) {
  return <RadialChart slices={slices} inner={0} />;
}
