import { invoke } from "@tauri-apps/api/core";

/** Aggregate of one indicator across a project's plots. */
export interface IndicatorAggregate {
  indicator: string;
  unit: string | null;
  plots: number;
  mean: number;
  min: number;
  max: number;
}

export interface PluginUsage {
  pluginId: string;
  images: number;
  lastRunAt: string | null;
}

export interface ComparisonRecord {
  id: string;
  beforeDate: string;
  afterDate: string;
  intervalDays: number;
  metric: string;
  beforeValue: number;
  afterValue: number;
  delta: number;
  magnitude: number;
  magnitudeLevel: "rendah" | "sedang" | "tinggi";
  createdAt: string;
}

export interface ProjectDashboard {
  projectId: string;
  sessions: number;
  images: number;
  geotaggedImages: number;
  plots: number;
  plotAreaHa: number;
  pluginResults: number;
  indicatorAggregates: IndicatorAggregate[];
  plugins: PluginUsage[];
  latestComparison: ComparisonRecord | null;
}

export interface PluginResultRow {
  imageId: string;
  filePath: string;
  pluginId: string;
  metric: string;
  unit: string | null;
  value: number | null;
  textValue: string | null;
  createdAt: string;
}

const PROJECT_KEY = "m1_last_project_id";

/** Project currently selected in Module 1. */
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

export function getProjectDashboard(projectId: string): Promise<ProjectDashboard> {
  return invoke<ProjectDashboard>("get_project_dashboard", { projectId });
}

export function listDashboardPluginResults(
  projectId: string,
  limit = 200,
): Promise<PluginResultRow[]> {
  return invoke<PluginResultRow[]>("list_dashboard_plugin_results", { projectId, limit });
}

export function exportDashboardCsv(projectId: string): Promise<string> {
  return invoke<string>("export_dashboard_plot_csv", { projectId });
}

export function exportDashboardGeojson(projectId: string): Promise<string> {
  return invoke<string>("export_dashboard_plots_geojson", { projectId });
}

export function exportDashboardJson(projectId: string): Promise<string> {
  return invoke<string>("export_dashboard_json", { projectId });
}

/** Human-readable metric value. */
export function formatMetric(row: PluginResultRow): string {
  if (row.value !== null) {
    const digits = Math.abs(row.value) >= 100 ? 0 : 2;
    return `${row.value.toFixed(digits)}${row.unit ? ` ${row.unit}` : ""}`;
  }
  return row.textValue ?? "";
}
