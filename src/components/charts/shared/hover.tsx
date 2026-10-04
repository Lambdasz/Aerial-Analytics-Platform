// hover.tsx

import { useState, type MouseEvent } from "react";

import type { Plot } from "./geometry";

export interface Pointer {
  readonly x: number;
  readonly y: number;
}

export interface ColumnHover extends Pointer {
  readonly index: number;
}

export function pointerIn(event: MouseEvent<SVGElement>): Pointer | null {
  const svg = event.currentTarget.ownerSVGElement;
  if (!svg) {
    return null;
  }
  const box = svg.getBoundingClientRect();
  return { x: event.clientX - box.left, y: event.clientY - box.top };
}

export function useColumnHover(plot: Plot, count: number) {
  const [hover, setHover] = useState<ColumnHover | null>(null);

  const bind = {
    onMouseMove: (event: MouseEvent<SVGElement>) => {
      const pointer = pointerIn(event);
      if (!pointer || count === 0) {
        return;
      }
      const band = plot.width / count;
      const index = Math.min(count - 1, Math.max(0, Math.floor((pointer.x - plot.left) / band)));
      setHover({ ...pointer, index });
    },
    onMouseLeave: () => {
      setHover(null);
    },
  };

  return { hover, bind };
}
