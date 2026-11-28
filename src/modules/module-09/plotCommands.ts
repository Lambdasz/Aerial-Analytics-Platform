import { invoke } from "@tauri-apps/api/core";
import { isTauri } from "@tauri-apps/api/core";

import type { AoiFeature } from "../module-03/types/aoi";
import { errorMessage } from "./plotAnalytics";

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

/** Project id from the stored value: a bare id string, or an object with `id`. Pure. */
const projectIdFrom = (parsed: unknown): string | null =>
  [
    parsed,
    parsed !== null && typeof parsed === "object" && "id" in parsed
      ? (parsed as { id?: unknown }).id
      : null,
  ].find((candidate): candidate is string => typeof candidate === "string" && candidate !== "") ??
  null;

/** Project currently selected in Module 1, if the app recorded one. */
export function currentProjectId(): string | null {
  try {
    return projectIdFrom(JSON.parse(window.localStorage.getItem(PROJECT_KEY) || "null"));
  } catch {
    return null;
  }
}

export function listPlots(projectId: string | null): Promise<PlotRecord[]> {
  return isTauri() ? invoke<PlotRecord[]>("list_plots", { projectId }) : Promise.resolve([]);
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

/**
 * Creates one plot per AOI, one after another (FR-9.1.1). A failing AOI does
 * not stop the others; resolves to the failure messages.
 */
export function createPlotsFromAois(aois: readonly AoiFeature[]): Promise<string[]> {
  return aois.reduce<Promise<string[]>>(
    (previous, aoi) =>
      previous.then((failures) =>
        createPlot(
          aoi.properties.name,
          aoi.geometry,
          currentProjectId(),
          null,
          aoi.properties.aoi_id,
        )
          .then(() => failures)
          .catch((err) => [...failures, `${aoi.properties.name}: ${errorMessage(err)}`]),
      ),
    Promise.resolve([]),
  );
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

const AREA_FORMATS: ReadonlyArray<{
  applies: (areaHa: number) => boolean;
  format: (areaM2: number, areaHa: number) => string;
}> = [
  { applies: (areaHa) => areaHa >= 1, format: (_, areaHa) => `${areaHa.toFixed(2)} ha` },
  {
    applies: (areaHa) => areaHa >= 0.01,
    format: (_, areaHa) => `${(areaHa * 10_000).toFixed(0)} m²`,
  },
  { applies: () => true, format: (areaM2) => `${areaM2.toFixed(1)} m²` },
];

/** Area formatted for display: m² below a hectare, ha above. */
export function formatArea(areaM2: number, areaHa: number): string {
  return (AREA_FORMATS.find((rule) => rule.applies(areaHa)) ?? AREA_FORMATS[2]).format(
    areaM2,
    areaHa,
  );
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
  return isTauri()
    ? invoke<PlotIndicator[]>("list_plot_indicators", { plotId, projectId })
    : Promise.resolve([]);
}

/** Number with 0 digits from 100 up, 2 below, plus the unit. */
const formatIndicatorNumber = (value: number, unit: string | null): string =>
  `${value.toFixed(Math.abs(value) >= 100 ? 0 : 2)}${unit ? ` ${unit}` : ""}`;

/** Human-readable indicator value. */
export function formatIndicator(indicator: PlotIndicator): string {
  return indicator.value !== null
    ? formatIndicatorNumber(indicator.value, indicator.unit)
    : (indicator.textValue ?? "");
}

/** Publishes all plots of a project as one Map Explorer layer. */
export function publishPlotsLayer(
  projectId: string | null,
  layerName: string | null = null,
): Promise<{ layer_name: string }> {
  return invoke<{ layer_name: string }>("publish_plots_layer", { projectId, layerName });
}

export interface AnalysePlotsOrthoPayload {
  orthoId: string;
  plotIds: string[];
  pluginId: string;
  parameters?: Record<string, unknown>;
}

export interface PlotAnalysisOutcome {
  successfulPlots: number;
  failedPlots: string[];
  totalSlicesProcessed: number;
}

export interface PlotAnalysisProgressEvent {
  stage: string;
  plotIndex: number;
  totalPlots: number;
  plotName: string;
  sliceIndex: number;
  totalSlices: number;
  percent: number;
  message: string;
}

/** Runs a plugin over plots using a georeferenced orthomosaic raster with tiled slicing. */
export function analysePlotsFromOrtho(
  payload: AnalysePlotsOrthoPayload,
): Promise<PlotAnalysisOutcome> {
  return invoke<PlotAnalysisOutcome>("analyse_plots_from_ortho", { payload });
}
