// geometry.ts

export interface Plot {
  readonly left: number;
  readonly top: number;
  readonly width: number;
  readonly height: number;
}

export interface Point {
  readonly x: number;
  readonly y: number;
}

export function plotArea(width: number, height: number): Plot {
  const left = 48;
  const top = 12;
  const right = 12;
  const bottom = 28;
  return {
    left,
    top,
    width: Math.max(0, width - left - right),
    height: Math.max(0, height - top - bottom),
  };
}

function f(value: number): string {
  return value.toFixed(2);
}

function straightPath(points: readonly Point[]): string {
  return points.map((p, i) => `${i === 0 ? "M" : "L"} ${f(p.x)} ${f(p.y)}`).join(" ");
}

function monotonePath(points: readonly Point[]): string {
  const n = points.length;
  const slopes: number[] = [];
  for (let i = 0; i < n - 1; i += 1) {
    const dx = points[i + 1].x - points[i].x;
    slopes.push(dx === 0 ? 0 : (points[i + 1].y - points[i].y) / dx);
  }
  const tangents: number[] = new Array<number>(n).fill(0);
  tangents[0] = slopes[0];
  tangents[n - 1] = slopes[n - 2];
  for (let i = 1; i < n - 1; i += 1) {
    tangents[i] = slopes[i - 1] * slopes[i] <= 0 ? 0 : (slopes[i - 1] + slopes[i]) / 2;
  }
  for (let i = 0; i < n - 1; i += 1) {
    if (slopes[i] === 0) {
      tangents[i] = 0;
      tangents[i + 1] = 0;
      continue;
    }
    const a = tangents[i] / slopes[i];
    const b = tangents[i + 1] / slopes[i];
    const sum = a * a + b * b;
    if (sum > 9) {
      const scale = 3 / Math.sqrt(sum);
      tangents[i] = scale * a * slopes[i];
      tangents[i + 1] = scale * b * slopes[i];
    }
  }
  let path = `M ${f(points[0].x)} ${f(points[0].y)}`;
  for (let i = 0; i < n - 1; i += 1) {
    const dx = (points[i + 1].x - points[i].x) / 3;
    path += ` C ${f(points[i].x + dx)} ${f(points[i].y + tangents[i] * dx)}`;
    path += ` ${f(points[i + 1].x - dx)} ${f(points[i + 1].y - tangents[i + 1] * dx)}`;
    path += ` ${f(points[i + 1].x)} ${f(points[i + 1].y)}`;
  }
  return path;
}

export function linePath(points: readonly Point[], smooth: boolean): string {
  if (points.length === 0) {
    return "";
  }
  return smooth && points.length > 2 ? monotonePath(points) : straightPath(points);
}

export function areaPath(points: readonly Point[], smooth: boolean, baseline: number): string {
  if (points.length === 0) {
    return "";
  }
  const first = points[0];
  const last = points[points.length - 1];
  return `${linePath(points, smooth)} L ${f(last.x)} ${f(baseline)} L ${f(first.x)} ${f(baseline)} Z`;
}

function polar(cx: number, cy: number, radius: number, angle: number): Point {
  return { x: cx + radius * Math.cos(angle), y: cy + radius * Math.sin(angle) };
}

function sector(
  cx: number,
  cy: number,
  outer: number,
  inner: number,
  start: number,
  end: number,
): string {
  const large = end - start > Math.PI ? 1 : 0;
  const a = polar(cx, cy, outer, start);
  const b = polar(cx, cy, outer, end);
  if (inner <= 0) {
    return `M ${f(cx)} ${f(cy)} L ${f(a.x)} ${f(a.y)} A ${f(outer)} ${f(outer)} 0 ${large} 1 ${f(b.x)} ${f(b.y)} Z`;
  }
  const c = polar(cx, cy, inner, end);
  const d = polar(cx, cy, inner, start);
  return `M ${f(a.x)} ${f(a.y)} A ${f(outer)} ${f(outer)} 0 ${large} 1 ${f(b.x)} ${f(b.y)} L ${f(c.x)} ${f(c.y)} A ${f(inner)} ${f(inner)} 0 ${large} 0 ${f(d.x)} ${f(d.y)} Z`;
}

export function sectorPath(
  cx: number,
  cy: number,
  outer: number,
  inner: number,
  start: number,
  end: number,
): string {
  if (end - start >= Math.PI * 2 - 1e-6) {
    const middle = start + Math.PI;
    return `${sector(cx, cy, outer, inner, start, middle)} ${sector(cx, cy, outer, inner, middle, end)}`;
  }
  return sector(cx, cy, outer, inner, start, end);
}

export function pointAt(cx: number, cy: number, radius: number, angle: number): Point {
  return polar(cx, cy, radius, angle);
}
