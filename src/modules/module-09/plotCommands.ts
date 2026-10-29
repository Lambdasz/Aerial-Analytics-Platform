import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@tauri-apps/api/core";

/** A stored plot boundary with its geodesic metrics. */
export interface PlotRecord {
  id: string;
  projectId: string | null;
  sessionId: string | null;
  name: string;
  geometry: { type: "Polygon"; coordinates: number[][][] };
  sourceAoiId: string | null;
  areaM2: number;
  areaHa: number;
  perimeterM: number;
  createdAt: string;
  updatedAt: string;
}

export interface PlotImportOutcome {
  plots: PlotRecord[];
  skipped: string[];
}

const PROJECT_KEY = "m1_last_project_id";

/** Project currently selected in Module 1, if the app recorded one. */
export function currentProjectId(): string | null {
  try {
    const raw = window.localStorage.getItem(PROJECT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as unknown;
    if (typeof parsed === "string" && parsed) return parsed;
    if (parsed && typeof parsed === "object" && "id" in parsed) {
      const id = (parsed as { id?: unknown }).id;
      if (typeof id === "string" && id) return id;
    }
    return null;
  } catch {
    return null;
  }
}

export function listPlots(projectId: string | null): Promise<PlotRecord[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<PlotRecord[]>("list_plots", { projectId });
}

/** GeoJSON Polygon geometry accepted by `create_plot`. */
export interface PlotGeometry {
  type: "Polygon";
  coordinates: number[][][];
}

export function createPlot(
  name: string,
  geometry: PlotGeometry,
  projectId: string | null,
  sessionId: string | null = null,
  sourceAoiId: string | null = null,
): Promise<PlotRecord> {
  return invoke<PlotRecord>("create_plot", {
    name,
    geometry,
    projectId,
    sessionId,
    sourceAoiId,
  });
}

export function renamePlot(plotId: string, name: string): Promise<PlotRecord> {
  return invoke<PlotRecord>("rename_plot", { plotId, name });
}

export function deletePlot(plotId: string): Promise<void> {
  return invoke<void>("delete_plot", { plotId });
}

export function importPlots(path: string, projectId: string | null): Promise<PlotImportOutcome> {
  return invoke<PlotImportOutcome>("import_plots_into_project", { path, projectId });
}

/** Area formatted for display: m² below a hectare, ha above. */
export function formatArea(areaM2: number, areaHa: number): string {
  if (areaHa >= 1) return `${areaHa.toFixed(2)} ha`;
  if (areaHa >= 0.01) return `${(areaHa * 10_000).toFixed(0)} m²`;
  return `${areaM2.toFixed(1)} m²`;
}

/** Perimeter formatted for display. */
export function formatPerimeter(perimeterM: number): string {
  return perimeterM >= 1000 ? `${(perimeterM / 1000).toFixed(2)} km` : `${perimeterM.toFixed(0)} m`;
}

/** Stored indicator row (latest value per plot/plugin/key). */
export interface PlotIndicator {
  plotId: string;
  pluginId: string;
  executionId: string | null;
  indicator: string;
  unit: string | null;
  value: number | null;
  textValue: string | null;
  createdAt: string;
}

export function savePlotIndicators(
  plotId: string,
  pluginId: string,
  executionId: string | null,
  metrics: Record<string, unknown>,
): Promise<number> {
  return invoke<number>("save_plot_indicators", { plotId, pluginId, executionId, metrics });
}

export function listPlotIndicators(
  plotId: string | null,
  projectId: string | null,
): Promise<PlotIndicator[]> {
  if (!isTauri()) return Promise.resolve([]);
  return invoke<PlotIndicator[]>("list_plot_indicators", { plotId, projectId });
}

/** Human-readable indicator value. */
export function formatIndicator(indicator: PlotIndicator): string {
  if (indicator.value !== null) {
    const digits = Math.abs(indicator.value) >= 100 ? 0 : 2;
    return `${indicator.value.toFixed(digits)}${indicator.unit ? ` ${indicator.unit}` : ""}`;
  }
  return indicator.textValue ?? "";
}

/** Publishes all plots of a project as one Map Explorer layer. */
export function publishPlotsLayer(
  projectId: string | null,
  layerName: string | null = null,
): Promise<{ layer_name: string }> {
  return invoke<{ layer_name: string }>("publish_plots_layer", { projectId, layerName });
}
