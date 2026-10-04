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
