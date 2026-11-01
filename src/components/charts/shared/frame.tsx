// frame.tsx

import { useEffect, useRef, useState, type ReactNode } from "react";

export interface ChartSize {
  readonly width: number;
  readonly height: number;
}

export interface ChartFrameProps {
  readonly children?: (size: ChartSize) => ReactNode;
}

export function ChartFrame({ children }: ChartFrameProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState<ChartSize>({ width: 0, height: 0 });

  useEffect(() => {
    const element = ref.current;
    if (!element) {
      return;
    }
    const observer = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      setSize({ width, height });
    });
    observer.observe(element);
    return () => {
      observer.disconnect();
    };
  }, []);

  const ready = size.width > 0 && size.height > 0;

  return (
    <div className="chart_frame" ref={ref}>
      {ready && children ? children(size) : null}
    </div>
  );
}
