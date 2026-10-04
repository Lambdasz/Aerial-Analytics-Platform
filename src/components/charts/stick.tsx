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
